import Foundation
import Combine
import Dispatch
import Darwin

public enum PetSource: String, CaseIterable, Identifiable {
    case wild, demo, live
    public var id: String { rawValue }
    public var label: String { switch self { case .wild: return "Wild"; case .demo: return "Event demo"; case .live: return "Live" } }
}

/// Thin host: settings, fixed-rate ticks, lifecycle and file delivery. The shared
/// world owns behaviour, accepted telemetry, interactions and score scheduling.
@MainActor public final class PetHost: ObservableObject {
    @Published public private(set) var core: PetNativeCore?
    @Published public private(set) var message = ""
    @Published public private(set) var persistenceMessage = ""
    @Published public var source: PetSource = .wild { didSet { if source != oldValue { save(); defaults.set(source.rawValue, forKey: "pet.source"); restart() } } }
    @Published public var still = false { didSet { defaults.set(still, forKey: "pet.still") } }
    @Published public var sound = false { didSet { defaults.set(sound, forKey: "pet.sound"); configureSound() } }
    public var systemReducedMotion = false
    private let points: [(Double, Double)]
    private let bundle: Bundle
    private let defaults: UserDefaults
    private let storageDirectory: URL
    private var store: PetHabitatStore?
    private var migratingLegacy = false
    private let audio = PetAudioOutput()
    private var timer: Timer?
    private var monitor: DispatchSourceFileSystemObject?
    private var fileMonitor: DispatchSourceFileSystemObject?
    private var lastPacket: String?
    private var lastSourceSequence = -1
    private var fileIdentity: UInt64?
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
    public func suspend(_ value: Bool) { paused = value; if value { save(); audio.stop() } else { configureSound() } }
    public func restart() {
        audio.stop(); restoring = false; failed = false; monitor?.cancel(); fileMonitor?.cancel(); monitor = nil; fileMonitor = nil; lastPacket = nil; lastSourceSequence = -1; fileIdentity = nil
        store = nil; persistenceMessage = ""; migratingLegacy = false; count = 0
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
                saved = try files.load(); store = files
            } catch { persistenceMessage = "Habitat storage is unavailable. Existing files were kept. This visit stays in memory." }
            let legacy = source == .wild && saved == nil && store != nil
            let interactions = legacy ? defaults.string(forKey: "pet.interactions") ?? "[]" : "[]"
            let restoredCore: PetNativeCore
            if let saved {
                do { restoredCore = try PetNativeCore(points: points, bundle: script, live: source == .live, saved: saved) }
                catch {
                    store = nil; persistenceMessage = "The habitat could not be restored. Its saved file was kept. This visit stays in memory."
                    restoredCore = try PetNativeCore(points: points, bundle: script, tape: tape, live: source == .live)
                }
            } else { restoredCore = try PetNativeCore(points: points, bundle: script, tape: tape, interactions: interactions, live: source == .live) }
            core = restoredCore
            message = source == .wild ? "Simulated creature" : source == .demo ? "Synthetic telemetry" : "Waiting for local telemetry"
            if source == .live, let stateURL { watch(stateURL) }
            if legacy {
                // Only pre-checkpoint preferences need historical simulation.
                // The first successful atomic save retires both legacy keys.
                migratingLegacy = defaults.object(forKey: "pet.elapsed") != nil || defaults.object(forKey: "pet.interactions") != nil
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
    public func setLiveFile(_ url: URL) { stateURL = url; if source == .live { monitor?.cancel(); fileMonitor?.cancel(); watch(url) } }
    public func interact(food: Bool) {
        do { try core?.interact(food: food); save() } catch { message = error.localizedDescription }
    }
    public func exportRecording(to url: URL) throws {
        guard let core else { throw PetCoreError.invalid("No pet recording is available.") }
        try core.recording().write(to: url, options: .atomic)
    }
    private func configureSound() {
        do { try audio.setEnabled(sound && !paused && !failed && !restoring, simulationTime: (core?.frame.timeMs ?? 0) / 1000) }
        catch { message = "Sound unavailable: \(error.localizedDescription)" }
    }
    private func tick() {
        guard !paused, !failed, !restoring, let core else { return }
        do {
            let f = try core.tick(motion: !(still || systemReducedMotion))
            try audio.present(f.voices, core: core)
            count += 1; if count % 150 == 0 { save() }
            objectWillChange.send()
        } catch { audio.stop(); failed = true; message = error.localizedDescription }
    }
    private func save() {
        guard let core, let store, !restoring, !failed else { return }
        do {
            try store.save(core.recording(checkpoint: true))
            if migratingLegacy {
                defaults.removeObject(forKey: "pet.interactions"); defaults.removeObject(forKey: "pet.elapsed"); migratingLegacy = false
            }
        } catch {
            self.store = nil
            persistenceMessage = "The habitat could not be saved. Its previous file was kept. This visit stays in memory."
        }
    }
    private func watch(_ url: URL) {
        // Watch the directory so atomic file replacement and initial creation work.
        let descriptor = open(url.deletingLastPathComponent().path, O_EVTONLY)
        guard descriptor >= 0 else { message = "Create the telemetry directory, then select Live again."; return }
        let source = DispatchSource.makeFileSystemObjectSource(fileDescriptor: descriptor, eventMask: [.write, .rename, .delete], queue: .main)
        source.setEventHandler { [weak self] in Task { @MainActor in self?.watchContents(url) } }
        source.setCancelHandler { close(descriptor) }; monitor = source; source.resume(); watchContents(url)
    }
    private func watchContents(_ url: URL) {
        guard source == .live, stateURL == url else { return }
        fileMonitor?.cancel(); fileMonitor = nil
        let identity = (try? FileManager.default.attributesOfItem(atPath: url.path)[.systemFileNumber] as? NSNumber)?.uint64Value
        if identity != fileIdentity { lastSourceSequence = -1; lastPacket = nil; fileIdentity = identity }
        let descriptor = open(url.path, O_EVTONLY)
        guard descriptor >= 0 else { message = "Telemetry unavailable · unobserved"; return }
        let source = DispatchSource.makeFileSystemObjectSource(fileDescriptor: descriptor, eventMask: [.write, .rename, .delete], queue: .main)
        source.setEventHandler { [weak self] in Task { @MainActor in self?.readPacket(url) } }
        source.setCancelHandler { close(descriptor) }; fileMonitor = source; source.resume(); readPacket(url)
    }
    private func readPacket(_ url: URL) {
        guard source == .live, stateURL == url else { return }
        do {
            let handle = try FileHandle(forReadingFrom: url); defer { try? handle.close() }
            let size = try handle.seekToEnd(); try handle.seek(toOffset: size > 262_144 ? size - 262_144 : 0)
            let data = try handle.readToEnd() ?? Data()
            guard data.last == 10, let text = String(data: data, encoding: .utf8), let last = text.split(separator: "\n").last else { return }
            let packet = String(last); if packet == lastPacket { return }
            guard let value = try JSONSerialization.jsonObject(with: Data(packet.utf8)) as? [String: Any], let sequence = value["sequence"] as? Int else { throw PetCoreError.invalid("Invalid telemetry packet.") }
            if sequence <= lastSourceSequence { return }
            try core?.accept(packet: packet); lastPacket = packet; lastSourceSequence = sequence; message = "Local telemetry connected"
        } catch { message = "Telemetry unavailable · unobserved" }
        // Missing/unchanged input never falls back to a demo. The recorded 400ms
        // packet expires in the core and subsequent ticks are visibly unobserved.
    }
}
