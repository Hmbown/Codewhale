import Foundation
import Darwin

@main struct AppleCheckpointProof {
    static func require(_ condition: @autoclosure () throws -> Bool, _ message: String) throws {
        if try !condition() { throw PetCoreError.invalid(message) }
    }
    static func reject(_ action: () throws -> Void, _ message: String) throws {
        do { try action() } catch { return }
        throw PetCoreError.invalid(message)
    }
    static func same(_ a: Data, _ b: Data) throws -> Bool {
        let left = try JSONSerialization.jsonObject(with: a) as! NSDictionary
        return try left.isEqual(JSONSerialization.jsonObject(with: b))
    }
    @MainActor static func main() async throws {
        let args = CommandLine.arguments
        let pet = URL(fileURLWithPath: args[1]), out = URL(fileURLWithPath: args[2])
        let bundle = Bundle(url: pet.appendingPathComponent("macos/CodewhalePet.app"))!
        let script = bundle.url(forResource: "pet-native", withExtension: "js")!
        let tape = try String(contentsOf: bundle.url(forResource: "demo", withExtension: "jsonl")!, encoding: .utf8)
        let points = try String(contentsOf: pet.appendingPathComponent("public/whale-points.tsv"), encoding: .utf8).split(separator: "\n").map { line -> (Double, Double) in
            let v = line.split(separator: "\t").map { Double($0)! }; return (v[0], v[1])
        }
        for input in ["", tape] {
            let original = try PetNativeCore(points: points, bundle: script, tape: input)
            for i in 0..<750 {
                if i == 615 { try original.interact(food: true) }
                try original.tick(motion: i % 90 < 60)
            }
            try original.interact(food: false, x: -0.3, y: 0.2)
            let saved = try original.recording(checkpoint: true)
            let restored = try PetNativeCore(points: points, bundle: script, saved: saved)
            try require(petDigest(restored.sim) == original.frame.digest, "Swift pose changed at restore")
            try require(same(saved, restored.recording(checkpoint: true)), "World checkpoint changed at restore")
            for i in 0..<240 {
                let a = try original.tick(motion: i % 65 < 40), b = try restored.tick(motion: i % 65 < 40)
                try require(a.digest == b.digest && petDigest(restored.sim) == b.digest, "Restored Swift physics diverged at \(i)")
                try require(same(original.recording(checkpoint: true), restored.recording(checkpoint: true)), "Restored world, score or journal diverged at \(i)")
            }
            var bad = try JSONSerialization.jsonObject(with: saved) as! [String: Any]
            var checkpoint = bad["checkpoint"] as! [String: Any]
            checkpoint["random"] = -1; bad["checkpoint"] = checkpoint
            let damaged = try JSONSerialization.data(withJSONObject: bad)
            try reject({ _ = try PetNativeCore(points: points, bundle: script, saved: damaged) }, "Malformed checkpoint accepted")
            print("PASS exact Apple checkpoint + 240 mixed-motion continuation frames, interactions and score: \(input.isEmpty ? "wild" : "demo pod")")
        }
        let human = tape.split(separator: "\n").first { line in
            let value = try? JSONSerialization.jsonObject(with: Data(line.utf8)) as? [String: Any]
            return value?["channel"] as? String == "human" && value?["waiting"] as? Bool == true
        }!
        let live = try PetNativeCore(points: points, bundle: script, live: true)
        try live.accept(packet: String(human))
        for _ in 0..<12 { try live.tick(motion: true) }
        try require(live.frame.state.observed >= 0.92 && live.frame.needs != "none", "Live fixture never observed a human request")
        let resumed = try PetNativeCore(points: points, bundle: script, live: true, saved: live.recording(checkpoint: true))
        try require(resumed.frame.state.observed == 0 && resumed.frame.needs == "none", "Old live request survived restoration")
        try require(petDigest(resumed.sim) == resumed.frame.digest, "Live resume left Swift geometry behind")
        print("PASS native live resume: preserved history, unknown interval and no stale human request")

        let root = out.appendingPathComponent("apple-checkpoint-fixture-" + UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let files = try PetHabitatStore(directory: root, source: "wild")
        let stale = try PetHabitatStore(directory: root, source: "wild")
        try require(files.load() == nil && stale.load() == nil, "Fixture was not empty")
        try files.save(Data("first".utf8))
        try reject({ try stale.save(Data("stale".utf8)) }, "A stale writer overwrote the habitat")
        let path = root.appendingPathComponent("wild.json")
        try require(String(contentsOf: path, encoding: .utf8) == "first", "Stale writer changed the file")
        let permission = try FileManager.default.attributesOfItem(atPath: path.path)[.posixPermissions] as! NSNumber
        try require(permission.intValue == 0o600, "Habitat is not private")
        try Data("external".utf8).write(to: path)
        try reject({ try files.save(Data("lost".utf8)) }, "External edit was overwritten")
        let outside = root.appendingPathComponent("original")
        try FileManager.default.moveItem(at: path, to: outside)
        try FileManager.default.createSymbolicLink(at: path, withDestinationURL: outside)
        try reject({ _ = try files.load() }, "Symbolic link accepted")
        try FileManager.default.removeItem(at: path)
        try FileManager.default.linkItem(at: outside, to: path)
        try reject({ _ = try files.load() }, "Hard link accepted")
        try FileManager.default.removeItem(at: path)
        try Data(count: PetHabitatStore.limit + 1).write(to: path)
        try reject({ _ = try files.load() }, "Oversize file accepted")
        try FileManager.default.removeItem(at: root.appendingPathComponent("wild.lock"))
        let replacement = try PetHabitatStore(directory: root, source: "wild")
        _ = replacement
        try reject({ try files.save(Data("split lock".utf8)) }, "Replaced writer lock accepted")
        try require((try FileManager.default.attributesOfItem(atPath: path.path)[.size] as! NSNumber).intValue == PetHabitatStore.limit + 1, "Rejected input was modified")
        let fifo = try PetHabitatStore(directory: root, source: "demo")
        try FileManager.default.removeItem(at: root.appendingPathComponent("demo.lock"))
        try require(mkfifo(root.appendingPathComponent("demo.lock").path, 0o600) == 0, "Could not create the sealed FIFO fixture")
        try reject({ _ = try fifo.load() }, "FIFO replacement blocked or passed the lock guard")
        print("PASS native private storage: stale and external writers, symbolic/hard links, size bound, replaced lock and nonblocking FIFO rejection")

        let hostDir = root.appendingPathComponent("host")
        let suite = "dev.shannonlabs.pet-checkpoint-test." + UUID().uuidString
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        let host = PetHost(points: points, bundle: bundle, defaults: defaults, storageDirectory: hostDir)
        host.suspend(true)
        for _ in 0..<180 { try host.core!.tick(motion: true) }
        let wildTime = host.core!.frame.timeMs, wildDigest = petDigest(host.core!.sim)
        host.source = .demo
        for _ in 0..<120 { try host.core!.tick(motion: false) }
        let demoTime = host.core!.frame.timeMs
        host.source = .wild
        try require(host.core!.frame.timeMs == wildTime && petDigest(host.core!.sim) == wildDigest, "Source switch reset the wild habitat")
        host.source = .demo
        try require(host.core!.frame.timeMs == demoTime, "Source switch reset the demo habitat")
        host.suspend(true)
        let reopened = PetHost(points: points, bundle: bundle, defaults: defaults, storageDirectory: hostDir)
        reopened.suspend(true)
        try require(reopened.core!.frame.timeMs == demoTime, "New host did not reopen its selected source")
        print("PASS actual Apple host: separate wild/demo checkpoints survive source switching and a new host")

        let corruptDir = root.appendingPathComponent("corrupt")
        try FileManager.default.createDirectory(at: corruptDir, withIntermediateDirectories: true)
        let corrupt = Data("{invalid habitat}".utf8)
        try corrupt.write(to: corruptDir.appendingPathComponent("demo.json"))
        let recovery = PetHost(points: points, bundle: bundle, defaults: defaults, storageDirectory: corruptDir)
        recovery.suspend(true); recovery.interact(food: true)
        try require(recovery.core != nil && !recovery.persistenceMessage.isEmpty, "Invalid file has no recovery notice")
        try require(Data(contentsOf: corruptDir.appendingPathComponent("demo.json")) == corrupt, "Recovery overwrote the damaged habitat")
        print("PASS actual Apple host: damaged recording is retained after suspension and interaction")

        let legacyDir = root.appendingPathComponent("legacy")
        defaults.set("wild", forKey: "pet.source"); defaults.set(3.0, forKey: "pet.elapsed"); defaults.set("[]", forKey: "pet.interactions")
        let legacy = PetHost(points: points, bundle: bundle, defaults: defaults, storageDirectory: legacyDir)
        legacy.suspend(true)
        for _ in 0..<1000 {
            if FileManager.default.fileExists(atPath: legacyDir.appendingPathComponent("wild.json").path) { break }
            await Task.yield()
        }
        try require(legacy.core?.frame.timeMs == 3000, "Legacy migration lost its elapsed time")
        try require(FileManager.default.fileExists(atPath: legacyDir.appendingPathComponent("wild.json").path), "Legacy migration did not save a checkpoint")
        try require(defaults.object(forKey: "pet.elapsed") == nil && defaults.object(forKey: "pet.interactions") == nil, "Legacy preferences were not retired after atomic save")
        print("PASS actual Apple host: legacy preferences migrate once after a successful checkpoint save")

        let longDir = root.appendingPathComponent("long")
        try FileManager.default.createDirectory(at: longDir, withIntermediateDirectories: true)
        try FileManager.default.copyItem(at: out.appendingPathComponent("long-habitat.json"), to: longDir.appendingPathComponent("live.json"))
        defaults.set("live", forKey: "pet.source")
        let start = ContinuousClock.now
        let long = PetHost(points: points, bundle: bundle, defaults: defaults, storageDirectory: longDir)
        long.suspend(true)
        try require(long.core!.frame.timeMs >= 7_200_000 && long.core!.frame.state.observed == 0, "Large native habitat did not resume unknown")
        try require(long.persistenceMessage.isEmpty, "Large native habitat failed to save")
        print("PASS actual Apple host: two-hour 5.54 MB synthetic unknown habitat restored and saved in \(start.duration(to: .now))")
        print("PASS 8 Apple checkpoint workflows; fixtures retained at \(root.path)")
    }
}
