import SwiftUI
import AppKit
import ServiceManagement

@main
struct CodewhalePetApp: App {
    @StateObject private var host = PetHost(points: WHALE_POINTS)
    @State private var login = SMAppService.mainApp.status == .enabled
    @State private var notice = ""
    var body: some Scene {
        MenuBarExtra {
            VStack(spacing: 12) {
                HStack { Text("Codewhale").font(.headline); Spacer(); Text("A living field").font(.caption).foregroundStyle(.secondary) }
                PetHabitatView(host: host).frame(height: 250)
                Picker("World", selection: $host.source) { ForEach(PetSource.allCases) { Text($0.label).tag($0) } }.pickerStyle(.segmented)
                HStack {
                    Toggle("Still", isOn: $host.still)
                    Toggle("Launch at login", isOn: $login).onChange(of: login) { _, value in
                        do { if value { try SMAppService.mainApp.register() } else { try SMAppService.mainApp.unregister() }; notice = "" }
                        catch { notice = error.localizedDescription; login = SMAppService.mainApp.status == .enabled }
                    }
                }.font(.caption)
                if host.source == .live { Text("Local source: ~/.codewhale/pet-state").font(.caption2).textSelection(.enabled) }
                if !notice.isEmpty { Text(notice).font(.caption).foregroundStyle(.secondary) }
                HStack {
                    Button("Save replay…") {
                        let panel = NSSavePanel(); panel.nameFieldStringValue = "codewhale-pet-replay.json"
                        if panel.runModal() == .OK, let url = panel.url {
                            do { try host.exportRecording(to: url); notice = "Replay saved" } catch { notice = error.localizedDescription }
                        }
                    }
                    Spacer(); Button("Quit") { host.suspend(true); NSApplication.shared.terminate(nil) }
                }
            }.padding(16).frame(width: 390)
        } label: {
            PetMenuIcon(host: host)
                .onAppear { host.setLiveFile(URL(fileURLWithPath: NSHomeDirectory() + "/.codewhale/pet-state")) }
        }.menuBarExtraStyle(.window)
    }
}

private struct PetMenuIcon: View {
    @ObservedObject var host: PetHost
    var body: some View {
        Canvas { ctx, size in
            guard let core = host.core else { return }
            let lay = petLayout(w: size.width, h: size.height, state: core.frame.state), frame = core.sim.frame
            for q in core.sim.p {
                let dot = Path(ellipseIn: CGRect(x: lay.ox + q.x * lay.scale * lay.flipX, y: lay.oy + q.y * lay.scale, width: 0.7, height: 0.7))
                if frame.hollow { ctx.stroke(dot, with: .color(.primary.opacity(0.55)), lineWidth: 0.3) }
                else { ctx.fill(dot, with: .color(.primary.opacity(0.8))) }
            }
        }.frame(width: 24, height: 24)
            .accessibilityLabel(host.core.map { petA11yLabel(sim: $0.sim, state: $0.frame.state) } ?? "Codewhale habitat unavailable")
    }
}
