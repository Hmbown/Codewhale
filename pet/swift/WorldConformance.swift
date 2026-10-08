import Foundation

@main struct WorldConformance {
    @MainActor static func main() throws {
        let args = CommandLine.arguments
        guard args.count >= 4 else { throw PetCoreError.invalid("Usage: world-conformance pet-native.js points.tsv demo.jsonl [--still]") }
        let points = try String(contentsOfFile: args[2], encoding: .utf8).split(separator: "\n").map { line -> (Double, Double) in
            let v = line.split(separator: "\t").compactMap { Double($0) }
            guard v.count == 2 else { throw PetCoreError.invalid("Invalid points.") }; return (v[0], v[1])
        }
        let tape = try String(contentsOfFile: args[3], encoding: .utf8)
        let core = try PetNativeCore(points: points, bundle: URL(fileURLWithPath: args[1]), tape: tape)
        var voices = core.frame.voices.count
        for i in 0..<2400 {
            let frame = try core.tick(motion: !args.contains("--still"))
            let native = petDigest(core.sim)
            guard native == frame.digest else { throw PetCoreError.invalid("Frame \(i): Swift \(native) != shared world \(frame.digest)") }
            voices += frame.voices.count
            if i % 300 == 0 { print("f\(i) \(native) \(frame.behaviour) \(frame.needs)") }
        }
        print("PASS 2400 native world frames; \(voices) scheduled voices; \(args.contains("--still") ? "still" : "animated")")
    }
}
