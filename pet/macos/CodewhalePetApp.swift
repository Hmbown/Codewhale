import SwiftUI
import AppKit
import ServiceManagement

@main
struct CodewhalePetApp: App {
    @StateObject private var host = PetHost(points: WHALE_POINTS)
    @Environment(\.openWindow) private var openWindow
    @State private var login = SMAppService.mainApp.status == .enabled
    @State private var notice = ""
    @State private var openedCompanion = false
    @State private var pendingSource: PetSource?
    @State private var confirmLeave = false
    var body: some Scene {
        Window("Codewhale · Shared habitat", id: "habitat") {
            VStack(alignment: .leading, spacing: 14) {
                Text("A WHALE LIVING IN CODE").font(.caption.monospaced()).foregroundStyle(.secondary)
                PetSharedHabitat(host: host.shared, still: host.still)
                Toggle("Still", isOn: $host.still)
            }.padding(22).frame(minWidth: 420, minHeight: 360).preferredColorScheme(.dark)
        }.defaultSize(width: 740, height: 520)
        MenuBarExtra {
            VStack(spacing: 12) {
                HStack { Text("Codewhale").font(.headline); Spacer(); Text("A living field").font(.caption).foregroundStyle(.secondary) }
                Button("Open companion window") { openWindow(id: "habitat"); NSApplication.shared.activate(ignoringOtherApps: true) }
                PetHabitatView(host: host).frame(height: 250)
                Picker("World", selection: Binding(get: { host.source }, set: { next in
                    if !host.selectSource(next) { pendingSource = next; confirmLeave = true }
                })) { ForEach(PetSource.allCases) { Text($0.label).tag($0) } }.pickerStyle(.segmented)
                HStack {
                    Toggle("Still", isOn: $host.still)
                    Toggle("Launch at login", isOn: $login).onChange(of: login) { _, value in
                        do { if value { try SMAppService.mainApp.register() } else { try SMAppService.mainApp.unregister() }; notice = "" }
                        catch { notice = error.localizedDescription; login = SMAppService.mainApp.status == .enabled }
                    }
                }.font(.caption)
                if host.source == .file { Text("Local file study: ~/.codewhale/pet-state").font(.caption2).textSelection(.enabled) }
                if !notice.isEmpty { Text(notice).font(.caption).foregroundStyle(.secondary) }
                HStack {
                    Menu("Earlier recordings") {
                        ForEach(host.archives, id: \.self) { name in
                            Button(petArchiveLabel(name)) {
                                do {
                                    let data = try host.archivedRecording(name)
                                    let panel = NSSavePanel(); panel.nameFieldStringValue = "codewhale-pet-earlier.json"
                                    if panel.runModal() == .OK, let url = panel.url { try data.write(to: url, options: .atomic); notice = "Earlier recording saved." }
                                } catch { notice = error.localizedDescription }
                            }
                        }
                    }.disabled(host.archives.isEmpty)
                    Button("Save recording…") {
                        let panel = NSSavePanel(); panel.nameFieldStringValue = "codewhale-pet-replay.json"
                        if panel.runModal() == .OK, let url = panel.url {
                            if host.source == .live {
                                Task { do { try await host.shared.export().write(to: url, options: .atomic); notice = "Shared recording saved." } catch { notice = error.localizedDescription } }
                            } else { do { try host.exportRecording(to: url); notice = "Recording saved. Files over 8 MiB open in the browser." } catch { notice = error.localizedDescription } }
                        }
                    }
                    Spacer(); Button("Quit") { host.suspend(true); NSApplication.shared.terminate(nil) }
                }
            }.padding(16).frame(width: 390)
                .alert("This world has not been saved", isPresented: $confirmLeave) {
                    Button("Keep this world", role: .cancel) { pendingSource = nil }
                    Button("Leave without saving", role: .destructive) {
                        if let next = pendingSource { host.selectSource(next, discardingUnsaved: true) }
                        pendingSource = nil
                    }
                } message: { Text("Keep this world and use Save recording to retain the current visit. Leaving opens the other world and discards this visit’s unsaved progress.") }
        } label: {
            PetMenuIcon(host: host)
                .onAppear {
                    host.setLiveFile(URL(fileURLWithPath: NSHomeDirectory() + "/.codewhale/pet-state"))
                    if ProcessInfo.processInfo.arguments.contains("--companion") && !openedCompanion {
                        openedCompanion = true; openWindow(id: "habitat"); NSApplication.shared.activate(ignoringOtherApps: true)
                    }
                }
        }.menuBarExtraStyle(.window)
    }
}

private struct PetMenuIcon: View {
    @ObservedObject var host: PetHost
    var body: some View {
        Canvas { ctx, size in
            guard let core = host.core else {
                let path = Path(ellipseIn: CGRect(x:3,y:7,width:17,height:9));ctx.stroke(path,with:.color(.primary),lineWidth:1)
                var tail = Path();tail.move(to:CGPoint(x:18,y:11));tail.addLine(to:CGPoint(x:23,y:6));tail.addLine(to:CGPoint(x:22,y:14));ctx.stroke(tail,with:.color(.primary),lineWidth:1)
                return
            }
            let lay = petLayout(w: size.width, h: size.height, state: core.frame.state), frame = core.sim.frame
            for q in core.sim.p {
                let dot = Path(ellipseIn: CGRect(x: lay.ox + q.x * lay.scale * lay.flipX, y: lay.oy + q.y * lay.scale, width: 0.7, height: 0.7))
                if frame.hollow { ctx.stroke(dot, with: .color(.primary.opacity(0.55)), lineWidth: 0.3) }
                else { ctx.fill(dot, with: .color(.primary.opacity(0.8))) }
            }
        }.frame(width: 24, height: 24)
            .accessibilityLabel(host.core.map { petA11yLabel(sim: $0.sim, state: $0.frame.state) } ?? "Codewhale shared habitat")
    }
}
