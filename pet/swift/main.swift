// Runner for the Swift pet core — same contract as run-tape.ts / rs/main.rs.
//   swiftc -O PetSim.swift main.swift -o petsim
//   ./petsim                    — run the tape, print digests
//   ./petsim --render N out.png — CoreGraphics raster of tape frame N
import Foundation
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers

func findFile(_ name: String) -> String {
    for p in [name, "pet/\(name)", "swift/../\(name)", "../\(name)"] {
        if FileManager.default.fileExists(atPath: p) { return p }
    }
    fatalError("\(name) not found")
}
func loadPoints() -> [(Double, Double)] {
    (try! String(contentsOfFile: findFile("whale-points.tsv"), encoding: .utf8))
        .trimmingCharacters(in: .whitespacesAndNewlines)
        .split(separator: "\n").map { l in
            let c = l.split(separator: "\t").map { Double($0)! }
            return (c[0], c[1])
        }
}
func loadTape() -> [(Double, PetState)] {
    (try! String(contentsOfFile: tapePath, encoding: .utf8))
        .split(separator: "\n").compactMap { line -> (Double, PetState)? in
            let c = line.split(separator: "\t")
            if c.count < 10 || c[0] == "dt" { return nil }
            var st = PetState()
            st.activity = Double(c[1])!; st.coherence = Double(c[2])!; st.attention = Double(c[3])!
            st.channel = String(c[4]); st.observed = Double(c[5])!
            st.roamX = Double(c[6])!; st.roamY = Double(c[7])!; st.flip = Double(c[8])!; st.lit = Double(c[9])!
            return (Double(c[0])!, st)
        }
}

/// The same draw the SwiftUI view performs: petLayout() + one ellipse per
/// particle, fill or stroke depending on frame.hollow.
func renderPng(sim: PetSim, st: PetState, path: String, W: Int = 900, H: Int = 420) {
    let space = CGColorSpaceCreateDeviceRGB()
    let ctx = CGContext(data: nil, width: W, height: H, bitsPerComponent: 8, bytesPerRow: 0,
                        space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    ctx.setFillColor(CGColor(red: 0.024, green: 0.031, blue: 0.051, alpha: 1))
    ctx.fill(CGRect(x: 0, y: 0, width: W, height: H))
    let lay = petLayout(w: Double(W), h: Double(H), state: st)
    let f = sim.frame
    let color = CGColor(red: f.r / 255, green: f.g / 255, blue: f.b / 255, alpha: f.alpha)
    ctx.setFillColor(color); ctx.setStrokeColor(color); ctx.setLineWidth(1)
    for q in sim.p {
        let px = lay.ox + q.x * lay.scale * lay.flipX
        let py = Double(H) - (lay.oy + q.y * lay.scale)   // CG y is bottom-up
        let rect = CGRect(x: px - lay.dot / 2, y: py - lay.dot / 2, width: lay.dot, height: lay.dot)
        if f.hollow { ctx.strokeEllipse(in: rect) } else { ctx.fillEllipse(in: rect) }
    }
    let img = ctx.makeImage()!
    let dest = CGImageDestinationCreateWithURL(URL(fileURLWithPath: path) as CFURL, UTType.png.identifier as CFString, 1, nil)!
    CGImageDestinationAddImage(dest, img, nil)
    CGImageDestinationFinalize(dest)
    print("wrote \(path) · \(f.channel) · \(f.arch)\(f.hollow ? " · unobserved" : "")")
}

let args = CommandLine.arguments
let motion = !args.contains("--reduced-motion")
let tapePath = args.firstIndex(of: "--tape").map { args[$0 + 1] } ?? findFile("tape.tsv")
let sim = PetSim(points: loadPoints())

if args.count > 1, args[1] == "--render" {
    let target = Int(args[2])!
    let out = args.count > 3 ? args[3] : "frame.png"
    var st = PetState()
    for (i, (dt, s)) in loadTape().enumerated() {
        sim.step(dt: dt, state: s, motion: motion)
        st = s
        if i == target { break }
    }
    renderPng(sim: sim, st: st, path: out)
} else {
    var f = 0
    for (dt, st) in loadTape() {
        sim.step(dt: dt, state: st, motion: motion)
        if f % 30 == 0 {
            print(String(format: "f%04d %@ %@", f, petDigest(sim), st.channel))
        }
        f += 1
    }
    print("final \(petDigest(sim))")
}
