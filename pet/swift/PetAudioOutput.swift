import Foundation
import AVFoundation

/// Voice timing and samples come from the shared core; the native host only
/// schedules bounded buffers against AVAudioEngine's presentation clock.
@MainActor public final class PetAudioOutput {
    private let engine = AVAudioEngine()
    private var active: [AVAudioPlayerNode] = []
    private let format = AVAudioFormat(standardFormatWithSampleRate: 48_000, channels: 2)!
    public private(set) var enabled = false
    private var epoch: UInt64 = 0
    private var simulationStart = 0.0

    public init() {}
    public func setEnabled(_ value: Bool, simulationTime: Double) throws {
        stop()
        if value {
            // Materialize the output graph before starting. With no voices yet,
            // AVAudioEngine otherwise raises an Objective-C exception (not Error).
            _ = engine.mainMixerNode; _ = engine.outputNode
            engine.prepare()
            try engine.start(); epoch = mach_absolute_time(); simulationStart = simulationTime
        }
        enabled = value
    }
    public func stop() {
        for node in active { node.stop(); engine.detach(node) }
        active.removeAll(); engine.stop(); enabled = false
    }
    public func present(_ voices: [PetVoice], core: PetNativeCore) throws {
        guard enabled else { return }
        // A stopped/replaced replay cannot leave an old voice attached.
        active.removeAll { node in
            if !node.isPlaying { engine.detach(node); return true }; return false
        }
        for voice in voices {
            if active.count >= 32 { break }
            let start = Int((voice.start * 48_000).rounded(.down)), length = Int((voice.duration * 48_000).rounded(.up)) + 2
            let samples = try core.pcm(voice: voice, startSample: start, length: length, rate: 48_000)
            guard samples.count == 2, samples.allSatisfy({ $0.count == length }),
                  let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(length)), let channels = buffer.floatChannelData
            else { throw PetCoreError.invalid("Invalid pet PCM buffer.") }
            buffer.frameLength = AVAudioFrameCount(length)
            for c in 0..<2 { samples[c].withUnsafeBufferPointer { channels[c].update(from: $0.baseAddress!, count: length) } }
            let node = AVAudioPlayerNode(); engine.attach(node); engine.connect(node, to: engine.mainMixerNode, format: format)
            let elapsed = max(0, Double(start) / 48_000 - simulationStart + 0.1)
            let when = AVAudioTime(hostTime: epoch + AVAudioTime.hostTime(forSeconds: elapsed))
            let identity = ObjectIdentifier(node)
            node.scheduleBuffer(buffer, at: nil, options: [], completionCallbackType: .dataPlayedBack) { [weak self] _ in
                Task { @MainActor in
                    guard let self, let node = self.active.first(where: { ObjectIdentifier($0) == identity }) else { return }
                    node.stop(); self.engine.detach(node); self.active.removeAll { $0 === node }
                }
            }
            node.play(at: when); active.append(node)
        }
    }
}
