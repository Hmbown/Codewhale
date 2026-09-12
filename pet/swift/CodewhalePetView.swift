// CodewhalePetView.swift — the native iOS/macOS renderer for the pet.
//
// The view owns no simulation: the caller steps `sim` (e.g. from a
// TimelineView's onChange or a CADisplayLink) and passes the `PetState` that
// produced the frame — the same contract the ratatui widget and the Compose
// renderer use. Rendering is one Canvas pass:
//
//   * filled dot per particle, colour and alpha from sim.frame
//   * hollow frames stroke the dots instead of filling them
//   * a `.sensoryFeedback` is deliberately NOT used for state — the label row
//     carries the non-colour cue ("tool · strike · unobserved")
//
// Reduced motion: when `accessibilityReduceMotion` is on, step the sim with
// motion: false — the same reduced-motion contract as every other port.
// Requires iOS 15+ / macOS 12+ (SwiftUI Canvas).

import SwiftUI

public struct CodewhalePetView: View {
    public let sim: PetSim
    public let state: PetState
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    public init(sim: PetSim, state: PetState) {
        self.sim = sim
        self.state = state
    }

    public var body: some View {
        VStack(spacing: 0) {
            Canvas { ctx, size in
                let lay = petLayout(w: size.width, h: size.height, state: state)
                let f = sim.frame
                let color = Color(red: f.r / 255, green: f.g / 255, blue: f.b / 255)
                let d = lay.dot
                for (i, q) in sim.p.enumerated() {
                    let px = lay.ox + q.x * lay.scale * lay.flipX
                    let py = lay.oy + q.y * lay.scale
                    let rect = CGRect(x: px - d / 2, y: py - d / 2, width: d, height: d)
                    let dot = Path(ellipseIn: rect)
                    if f.hollow {
                        ctx.stroke(dot, with: .color(color.opacity(f.alpha)), lineWidth: 1)
                    } else {
                        ctx.fill(dot, with: .color(color.opacity(f.alpha)))
                    }
                    _ = i
                }
            }
            .accessibilityLabel(Text(petA11yLabel(sim: sim, state: state)))
            Text("\(sim.frame.channel) · \(sim.frame.arch)\(sim.frame.hollow ? " · unobserved" : "")")
                .font(.caption.monospaced())
                .foregroundStyle(.secondary)
        }
    }
}

/// VoiceOver gets the semantic frame, not "a whale animation".
public func petA11yLabel(sim: PetSim, state: PetState) -> String {
    let f = sim.frame
    var parts = ["Codewhale pet", f.channel, f.arch]
    if f.hollow { parts.append("unobserved") }
    if state.lit < 0.5 { parts.append("dozing") }
    if state.attention > 0.5 { parts.append("needs you") }
    return parts.joined(separator: ", ")
}
