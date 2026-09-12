import SwiftUI
import AVFoundation

@main struct CodewhalePetMobileApp: App {
    @StateObject private var host = PetHost(points: WHALE_POINTS)
    @Environment(\.scenePhase) private var phase
    var body: some Scene {
        WindowGroup {
            VStack(alignment: .leading, spacing: 22) {
                Text("CODEWHALE / WHALESONG").font(.caption.monospaced()).foregroundStyle(.secondary)
                Text("A living field.").font(.largeTitle.weight(.medium))
                Text("Activity sets the pace. Coherence holds the figure together. When it needs you, it turns toward you.").font(.subheadline).foregroundStyle(.secondary)
                PetHabitatView(host: host).frame(minHeight: 300, maxHeight: .infinity)
                Picker("World", selection: $host.source) { ForEach(PetSource.allCases) { Text($0.label).tag($0) } }.pickerStyle(.segmented)
                Toggle("Still", isOn: $host.still)
                Text(host.source == .live ? "Live input: pet-state in this app’s Documents folder." : "Hollow dots mean missing telemetry. Sleep is a separate dimmer.")
                    .font(.caption).foregroundStyle(.secondary)
            }.padding(24).preferredColorScheme(.dark)
                .onAppear {
                    host.setLiveFile(FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0].appendingPathComponent("pet-state"))
                    do { try AVAudioSession.sharedInstance().setCategory(.ambient, mode: .default); try AVAudioSession.sharedInstance().setActive(true) }
                    catch { /* Visual world remains available; audio output reports start errors. */ }
                }
                .onChange(of: phase) { _, value in host.suspend(value != .active) }
        }
    }
}
