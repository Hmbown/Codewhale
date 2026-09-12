import SwiftUI

public struct PetHabitatView: View {
    @ObservedObject var host: PetHost
    @Environment(\.accessibilityReduceMotion) private var reducedMotion
    public init(host: PetHost) { self.host = host }
    public var body: some View {
        VStack(spacing: 12) {
            if let core = host.core {
                ZStack {
                    LinearGradient(colors: [Color(red: 0.06, green: 0.15, blue: 0.20), Color(red: 0.03, green: 0.07, blue: 0.10)], startPoint: .top, endPoint: .bottom)
                    Canvas { ctx, size in
                        let frame = core.frame
                        var surface = Path(); surface.move(to: CGPoint(x: 12, y: 20 + frame.surface * 2)); surface.addLine(to: CGPoint(x: size.width - 12, y: 20 + frame.surface * 2))
                        ctx.stroke(surface, with: .color(.cyan.opacity(0.12)), lineWidth: 1)
                        for i in 0..<5 {
                            var path = Path()
                            for x in stride(from: 0.0, through: size.width, by: 8) {
                                let t = host.still || reducedMotion ? 0 : frame.timeMs / 1000
                                let y = size.height * 0.85 + sin(x / 90 + Double(i) + t * 0.18) * 6 + Double(i) * 4
                                if x == 0 { path.move(to: CGPoint(x: x, y: y)) } else { path.addLine(to: CGPoint(x: x, y: y)) }
                            }
                            ctx.stroke(path, with: .color(.cyan.opacity(0.025 + 0.015 * frame.caustic)), lineWidth: 1)
                        }
                        if let food = frame.food {
                            let rect = CGRect(x: size.width * (0.5 + food.x * 0.3), y: size.height * (0.5 + food.y * 0.3), width: 4, height: 4)
                            ctx.fill(Path(ellipseIn: rect), with: .color(.yellow.opacity(food.life * 0.6)))
                        }
                    }
                    CodewhalePetView(sim: core.sim, state: core.frame.state).padding(12)
                }
                .clipShape(RoundedRectangle(cornerRadius: 6))
                Text("\(core.frame.behaviour)\(core.frame.needs == "none" ? "" : " · awaiting input")")
                    .font(.caption.monospaced()).foregroundStyle(.secondary)
            } else { Text("Habitat unavailable").frame(maxWidth: .infinity, maxHeight: .infinity) }
            Text(host.message).font(.caption).foregroundStyle(.secondary)
            if !host.persistenceMessage.isEmpty { Text(host.persistenceMessage).font(.caption).foregroundStyle(.secondary) }
            HStack {
                Button("Focus") { host.interact(food: false) }
                Button("Pulse") { host.interact(food: true) }
                Toggle("Sound", isOn: $host.sound)
            }
        }
        .onAppear { host.systemReducedMotion = reducedMotion }
        .onChange(of: reducedMotion) { _, value in host.systemReducedMotion = value }
    }
}
