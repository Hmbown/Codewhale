import Foundation
import Combine
import Dispatch
import Darwin

public enum PetSource: String, CaseIterable, Identifiable {
    case wild, demo, live
    public var id: String { rawValue }
    public var label: String { switch self { case .wild: return "Wild"; case .demo: return "Event demo"; case .live: return "Live" } }
}

public func petArchiveLabel(_ name: String) -> String {
    let tick = Double(name.split(separator: "-").dropLast().last ?? "") ?? 0
    let seconds = Int(tick / 30)
    return String(format: "Through %d:%02d:%02d", seconds / 3600, seconds / 60 % 60, seconds % 60)
}

/// Thin host: settings, fixed-rate ticks, lifecycle and file delivery. The shared
/// world owns behaviour, accepted telemetry, interactions and score scheduling.
@MainActor public final class PetHost: ObservableObject {
    @Published public private(set) var core: PetNativeCore?
    @Published public private(set) var message = ""
    @Published public private(set) var persistenceMessage = ""
    @Published public private(set) var source: PetSource = .wild
    @Published public private(set) var archives: [String] = []
    @Published public var still = false { didSet { defaults.set(still, forKey: "pet.still") } }
    @Published public var sound = false { didSet { defaults.set(sound, forKey: "pet.sound"); configureSound() } }
    public var systemReducedMotion = false
    private let points: [(Double, Double)]
    private let bundle: Bundle
    private let defaults: UserDefaults
    private let storageDirectory: URL
    private var store: PetHabitatStore?
    private var archiveStore: PetHabitatStore?
    private var migratingLegacy = false
    private let audio = PetAudioOutput()
    private var timer: Timer?
    private var monitor: DispatchSourceFileSystemObject?
    private var fileMonitor: DispatchSourceFileSystemObject?
    private var count = 0
    private var stateURL: URL?
    private var paused = false
    private var failed = false
    private var restoring = false

    public init(points: [(Double, Double)], bundle: Bundle = .main, defaults: UserDefaults = .standard, storageDirectory: URL? = nil) {
        self.points = points; self.bundle = bundle; self.defaults = defaults
        self.storageDirectory = storageDirectory ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("CodewhalePet", isDirectory: true)
        source = PetSource(rawValue: defaults.string(forKey: "pet.source") ?? "wild") ?? .wild
        still = defaults.bool(forKey: "pet.still"); sound = defaults.bool(forKey: "pet.sound")
        restart()
        timer = Timer.scheduledTimer(withTimeInterval: 1.0 / 30, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.tick() }
        }
    }
    public func suspend(_ value: Bool) {
        let wasPaused = paused; paused = value
        if value {
            save(); audio.stop(); monitor?.cancel(); fileMonitor?.cancel(); monitor = nil; fileMonitor = nil
        } else {
            if wasPaused && source == .live {
                do { try core?.resumeLiveInput(); if let stateURL { watch(stateURL) }; objectWillChange.send() }
                catch { message = error.localizedDescription }
            }
            configureSound()
        }
    }
    @discardableResult public func selectSource(_ next: PetSource, discardingUnsaved: Bool = false) -> Bool {
        guard next != source else { return true }
        guard discardingUnsaved || save() else { return false }
        source = next; defaults.set(next.rawValue, forKey: "pet.source"); restart()
        return true
    }
    private func restart() {
        audio.stop(); restoring = false; failed = false; monitor?.cancel(); fileMonitor?.cancel(); monitor = nil; fileMonitor = nil
        store = nil; archiveStore = nil; archives = []; persistenceMessage = ""; migratingLegacy = false; count = 0
        do {
            guard let script = bundle.url(forResource: "pet-native", withExtension: "js") else { throw PetCoreError.invalid("The shared pet core is missing from this app.") }
            var tape = ""
            if source == .demo {
                guard let demo = bundle.url(forResource: "demo", withExtension: "jsonl") else { throw PetCoreError.invalid("The event demo is missing from this app.") }
                tape = try String(contentsOf: demo, encoding: .utf8)
            }
            var saved: Data?
            do {
                let files = try PetHabitatStore(directory: storageDirectory, source: source.rawValue)
                archiveStore = files; archives = (try? files.archives()) ?? []
                saved = try files.load(); store = files
            } catch { persistenceMessage = "Habitat storage is unavailable. Existing files were kept. This visit stays in memory." }
            let legacy = source == .wild && saved == nil && store != nil
            let hasLegacy = legacy && (defaults.object(forKey: "pet.elapsed") != nil || defaults.object(forKey: "pet.interactions") != nil)
            let interactions = legacy ? defaults.string(forKey: "pet.interactions") ?? "[]" : "[]"
            let restoredCore: PetNativeCore
            if let saved {
                do { restoredCore = try PetNativeCore(points: points, bundle: script, live: source == .live, saved: saved) }
                catch {
                    store = nil; persistenceMessage = "The habitat could not be restored. Its saved file was kept. This visit stays in memory."
                    restoredCore = try PetNativeCore(points: points, bundle: script, tape: tape, live: source == .live)
                }
            } else { restoredCore = try PetNativeCore(points: points, bundle: script, tape: tape, interactions: interactions, live: source == .live, expressionVersion: hasLegacy ? 1 : 2) }
            core = restoredCore
            message = source == .wild ? "Simulated creature" : source == .demo ? "Synthetic telemetry" : "Waiting for local telemetry"
            if source == .live, let stateURL { watch(stateURL) }
            if legacy {
                // Only pre-checkpoint preferences need historical simulation.
                // The first successful atomic save retires both legacy keys.
                migratingLegacy = hasLegacy
                let elapsed = defaults.double(forKey: "pet.elapsed")
                if elapsed.isFinite && elapsed > 0 && elapsed <= 86_400 {
                    restoring = true
                    Task { @MainActor [weak self, weak core] in
                        guard let self, let core else { return }
                        do {
                            for i in 0..<Int(elapsed * 30) {
                                guard self.core === core else { return }
                                try core.tick(motion: !(self.still || self.systemReducedMotion))
                                if i % 120 == 0 { self.message = "Restoring the habitat…"; self.objectWillChange.send(); await Task.yield() }
                            }
                            self.message = "Simulated creature"; self.restoring = false; self.save(); self.configureSound()
                        } catch { self.restoring = false; self.store = nil; self.message = error.localizedDescription }
                    }
                } else if !elapsed.isFinite || elapsed < 0 || elapsed > 86_400 {
                    store = nil; persistenceMessage = "The older habitat could not be restored. Its saved preferences were kept. This visit stays in memory."
                }
            }
            if !restoring { configureSound() }
        } catch { core = nil; message = error.localizedDescription }
    }
    public func setLiveFile(_ url: URL) {
        let changed = stateURL != url; stateURL = url
        if source == .live && !paused {
            monitor?.cancel(); fileMonitor?.cancel()
            do { if changed { try core?.resumeLiveInput() }; watch(url) }
            catch { message = error.localizedDescription }
        }
    }
    public func interact(food: Bool) {
        do { try core?.interact(food: food); save() } catch { message = error.localizedDescription }
    }
    public func exportRecording(to url: URL) throws {
        try recordingForExport().write(to: url, options: .atomic)
    }
    public func recordingForExport() throws -> Data {
        guard !restoring, let core else { throw PetCoreError.invalid("Wait for the habitat to finish opening before exporting.") }
        return try core.exportRecording()
    }
    public func archivedRecording(_ name: String) throws -> Data {
        guard let archiveStore else { throw PetCoreError.invalid("Habitat storage is unavailable.") }
        return try archiveStore.archivedRecording(name)
    }
    private func configureSound() {
        do { try audio.setEnabled(sound && !paused && !failed && !restoring, simulationTime: (core?.frame.timeMs ?? 0) / 1000) }
        catch { sound = false; message = "Sound unavailable: \(error.localizedDescription)" }
    }
    private func tick() {
        guard !paused, !failed, !restoring, let core else { return }
        do {
            let f = try core.tick(motion: !(still || systemReducedMotion))
            do { try audio.present(f.voices, core: core) }
            catch { sound = false; message = "Sound unavailable: \(error.localizedDescription)" }
            count += 1; if count % 150 == 0 { save() }
            objectWillChange.send()
        } catch { audio.stop(); failed = true; message = error.localizedDescription }
    }
    @discardableResult private func save() -> Bool {
        guard let core else { return true }
        guard let store, !restoring, !failed else { return false }
        do {
            if let next = try core.prepareSegment() {
                let archive = try core.exportRecording(completed: true)
                try store.save(next, archive: archive, tick: Int((core.frame.timeMs * 30 / 1000).rounded()))
                try core.commitSegment(); archives = (try? store.archives()) ?? archives
            } else { try store.save(core.recording(checkpoint: true)) }
            if migratingLegacy {
                defaults.removeObject(forKey: "pet.interactions"); defaults.removeObject(forKey: "pet.elapsed"); migratingLegacy = false
            }
            persistenceMessage = ""
            return true
        } catch {
            persistenceMessage = "The habitat could not be saved. Its previous file was kept. This visit stays in memory."
            return false
        }
    }
    private func watch(_ url: URL) {
        guard source == .live, stateURL == url, !paused else { return }
        // Watch the directory so atomic file replacement and initial creation work.
        let descriptor = open(url.deletingLastPathComponent().path, O_EVTONLY)
        guard descriptor >= 0 else { message = "Create the telemetry directory, then select Live again."; return }
        let source = DispatchSource.makeFileSystemObjectSource(fileDescriptor: descriptor, eventMask: [.write, .rename, .delete], queue: .main)
        source.setEventHandler { [weak self] in Task { @MainActor in self?.watchContents(url) } }
        source.setCancelHandler { close(descriptor) }; monitor = source; source.resume(); watchContents(url)
    }
    private func watchContents(_ url: URL) {
        guard source == .live, stateURL == url, !paused else { return }
        fileMonitor?.cancel(); fileMonitor = nil
        let descriptor = open(url.path, O_EVTONLY)
        guard descriptor >= 0 else { _ = try? core?.acceptLiveTail(""); message = "Telemetry unavailable · unobserved"; return }
        let source = DispatchSource.makeFileSystemObjectSource(fileDescriptor: descriptor, eventMask: [.write, .rename, .delete], queue: .main)
        source.setEventHandler { [weak self] in Task { @MainActor in self?.readPacket(url) } }
        source.setCancelHandler { close(descriptor) }; fileMonitor = source; source.resume(); readPacket(url)
    }
    private func readPacket(_ url: URL) {
        guard source == .live, stateURL == url, !paused else { return }
        do {
            let handle = try FileHandle(forReadingFrom: url); defer { try? handle.close() }
            let size = try handle.seekToEnd(); try handle.seek(toOffset: size > 262_144 ? size - 262_144 : 0)
            let data = try handle.read(upToCount: 262_144) ?? Data()
            try core?.acceptLiveTail(String(decoding: data, as: UTF8.self))
            message = "Following local telemetry"
        } catch { _ = try? core?.acceptLiveTail(""); message = "Telemetry unavailable · unobserved" }
        // Missing/unchanged input never falls back to a demo. The recorded 400ms
        // packet expires in the core and subsequent ticks are visibly unobserved.
    }
}
