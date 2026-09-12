import Foundation
import JavaScriptCore

public struct PetVoice: Codable {
    public let id: String, start: Double, duration: Double, frequency: Double, gain: Double, pan: Double, kind: String
}
public struct PetPeer: Decodable {
    public let id: String, slot: Int, phase: Double, present: Bool
}
public struct PetFood: Decodable { public let x: Double, y: Double, life: Double }
public struct PetWorldFrame: Decodable {
    public let timeMs: Double, behaviour: String, state: PetState, needs: String
    public let pod: [PetPeer], surface: Double, caustic: Double, food: PetFood?
    public let voices: [PetVoice], digest: String
}
public enum PetCoreError: Error, LocalizedError {
    case invalid(String)
    public var errorDescription: String? { switch self { case .invalid(let s): return s } }
}

/// JavaScriptCore runs the same event-independent world and score bundle as web.
/// Swift owns rasterization and audio output. No native telemetry reclassification.
@MainActor public final class PetNativeCore {
    public let sim: PetSim
    public private(set) var frame: PetWorldFrame
    private let context: JSContext
    private let world: JSValue
    private var failure: String?

    public init(points: [(Double, Double)], bundle: URL, tape: String = "", interactions: String = "[]", live: Bool = false, saved: Data? = nil, expressionVersion: Int = 2) throws {
        guard let context = JSContext() else { throw PetCoreError.invalid("Unable to create the pet runtime.") }
        self.context = context
        let script = try String(contentsOf: bundle, encoding: .utf8)
        context.evaluateScript(script)
        if let error = context.exception { throw PetCoreError.invalid(error.toString()) }
        let pointData = try JSONSerialization.data(withJSONObject: points.map { [$0.0, $0.1] })
        guard let constructor = context.objectForKeyedSubscript("PetNative"),
              let world = constructor.construct(withArguments: [String(decoding: pointData, as: UTF8.self), tape, interactions, live, expressionVersion]), context.exception == nil
        else { throw PetCoreError.invalid(context.exception?.toString() ?? "Invalid pet recording.") }
        if let saved {
            guard saved.count <= 8 * 1024 * 1024, let text = String(data: saved, encoding: .utf8) else { throw PetCoreError.invalid("Invalid pet habitat file.") }
            world.invokeMethod("restoreRecording", withArguments: [text])
            if let error = context.exception { throw PetCoreError.invalid(error.toString()) }
            if live {
                world.invokeMethod("resumeEngine", withArguments: [])
                if let error = context.exception { throw PetCoreError.invalid(error.toString()) }
            }
        }
        guard let text = world.invokeMethod("snapshot", withArguments: [])?.toString(), context.exception == nil
        else { throw PetCoreError.invalid(context.exception?.toString() ?? "Invalid pet recording.") }
        self.world = world
        self.frame = try JSONDecoder().decode(PetWorldFrame.self, from: Data(text.utf8))
        self.sim = PetSim(points: points, expressionVersion: expressionVersion)
        if saved != nil {
            struct Projection: Decodable { let sim: PetParticleCheckpoint }
            guard let text = world.invokeMethod("checkpoint", withArguments: [])?.toString(), context.exception == nil
            else { throw PetCoreError.invalid(context.exception?.toString() ?? "Unable to restore the particle renderer.") }
            try sim.restoreValidated(JSONDecoder().decode(Projection.self, from: Data(text.utf8)).sim)
            guard petDigest(sim) == frame.digest else { throw PetCoreError.invalid("The restored particle renderer differs from the shared world.") }
        }
        context.exceptionHandler = { [weak self] _, error in self?.failure = error?.toString() ?? "Pet runtime failed." }
    }

    @discardableResult public func tick(motion: Bool) throws -> PetWorldFrame {
        failure = nil
        guard let text = world.invokeMethod("step", withArguments: [1.0 / 30, motion])?.toString(), failure == nil
        else { throw PetCoreError.invalid(failure ?? "Unable to advance the pet.") }
        frame = try JSONDecoder().decode(PetWorldFrame.self, from: Data(text.utf8))
        let peers = frame.pod.filter { $0.present }
        let slots = peers.count >= 3 ? peers.map { ([0, 2, 4, 1, 3, 5][$0.slot], $0.phase) } : nil
        sim.step(dt: 1.0 / 30, state: frame.state, motion: motion, podSlots: slots)
        return frame
    }
    public func interact(food: Bool, x: Double = 0.2, y: Double = -0.15) throws {
        failure = nil
        world.invokeMethod("interact", withArguments: [food ? "food" : "attention", x, y])
        if let failure { throw PetCoreError.invalid(failure) }
    }
    public func interactions() throws -> String {
        failure = nil
        let value = world.invokeMethod("interactions", withArguments: [])?.toString()
        guard let value, failure == nil else { throw PetCoreError.invalid(failure ?? "Unable to save pet interactions.") }
        return value
    }
    public func accept(packet: String) throws {
        failure = nil
        world.invokeMethod("accept", withArguments: [packet])
        if let failure { throw PetCoreError.invalid(failure) }
    }
    public func recording(checkpoint: Bool = false) throws -> Data {
        failure = nil
        guard let value = world.invokeMethod("recording", withArguments: [checkpoint])?.toString(), failure == nil else {
            throw PetCoreError.invalid(failure ?? "Unable to save the pet recording.")
        }
        return Data(value.utf8)
    }
    public func prepareSegment() throws -> Data? {
        failure = nil
        guard let needed = world.invokeMethod("needsSegment", withArguments: []), failure == nil else { throw PetCoreError.invalid(failure ?? "Unable to inspect recording history.") }
        if !needed.toBool() { return nil }
        guard let text = world.invokeMethod("prepareSegment", withArguments: [])?.toString(), failure == nil else { throw PetCoreError.invalid(failure ?? "Unable to prepare recording history.") }
        let data = Data(text.utf8)
        guard data.count <= 8 * 1024 * 1024 else { throw PetCoreError.invalid("The active recording exceeds 8 MiB.") }
        return data
    }
    public func commitSegment() throws {
        failure = nil; world.invokeMethod("commitSegment", withArguments: [])
        if let failure { throw PetCoreError.invalid(failure) }
    }
    /// The host owns the world for this entire export. The private autosave and
    /// native import remain bounded to 8 MiB; larger exports open in the browser.
    public func exportRecording(completed: Bool = false) throws -> Data {
        var data = Data(), index = 0
        while true {
            failure = nil
            guard let value = world.invokeMethod("recordingChunk", withArguments: [index, completed]), failure == nil else {
                throw PetCoreError.invalid(failure ?? "Unable to export the pet recording.")
            }
            if value.isNull { return data }
            guard value.isString, let text = value.toString() else { throw PetCoreError.invalid("Invalid pet export chunk.") }
            let bytes = Data(text.utf8)
            guard data.count + bytes.count <= 64 * 1024 * 1024 else { throw PetCoreError.invalid("Recording exceeds the 64 MiB export limit. The current world was kept.") }
            data.append(bytes); index += 1
        }
    }
    public func pcm(voice: PetVoice, startSample: Int, length: Int, rate: Int) throws -> [[Float]] {
        let voices = String(decoding: try JSONEncoder().encode([voice]), as: UTF8.self)
        failure = nil
        let text = world.invokeMethod("pcm", withArguments: [voices, startSample, length, rate])?.toString()
        guard let text, failure == nil else { throw PetCoreError.invalid(failure ?? "Unable to render pet audio.") }
        return try JSONDecoder().decode([[Float]].self, from: Data(text.utf8))
    }
}
