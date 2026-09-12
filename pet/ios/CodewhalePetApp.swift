import SwiftUI
import AVFoundation
import UniformTypeIdentifiers

@main struct CodewhalePetMobileApp: App {
    @StateObject private var host = PetHost(points: WHALE_POINTS)
    @Environment(\.scenePhase) private var phase
    @State private var pendingSource: PetSource?
    @State private var confirmLeave = false
    @State private var exporting = false
    @State private var recording: PetRecordingDocument?
    @State private var notice = ""
    var body: some Scene {
        WindowGroup {
            VStack(alignment: .leading, spacing: 22) {
                Text("CODEWHALE / WHALESONG").font(.caption.monospaced()).foregroundStyle(.secondary)
                Text("A living field.").font(.largeTitle.weight(.medium))
                Text("A whale at rest. Knots, strands and branching paths as it works. The dots make the activity visible.").font(.subheadline).foregroundStyle(.secondary)
                PetHabitatView(host: host).frame(minHeight: 300, maxHeight: .infinity)
                Picker("World", selection: Binding(get: { host.source }, set: { next in
                    if !host.selectSource(next) { pendingSource = next; confirmLeave = true }
                })) { ForEach(PetSource.allCases) { Text($0.label).tag($0) } }.pickerStyle(.segmented)
                Toggle("Still", isOn: $host.still)
                Button("Save recording…") {
                    do { recording = PetRecordingDocument(data: try host.recordingForExport()); exporting = true }
                    catch { notice = error.localizedDescription }
                }.disabled(host.core == nil)
                if !notice.isEmpty { Text(notice).font(.caption).foregroundStyle(.secondary) }
                Text(host.source == .live ? "Live input: pet-state in this app’s Documents folder." : "Hollow dots mean missing telemetry. Sleep is a separate dimmer.")
                    .font(.caption).foregroundStyle(.secondary)
            }.padding(24).preferredColorScheme(.dark)
                .onAppear {
                    host.setLiveFile(FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0].appendingPathComponent("pet-state"))
                    do { try AVAudioSession.sharedInstance().setCategory(.ambient, mode: .default); try AVAudioSession.sharedInstance().setActive(true) }
                    catch { /* Visual world remains available; audio output reports start errors. */ }
                }
                .onChange(of: phase) { _, value in host.suspend(value != .active) }
                .fileExporter(isPresented: $exporting, document: recording, contentType: .json, defaultFilename: "codewhale-pet.json") { result in
                    switch result {
                    case .success: notice = "Recording saved, including this world’s current state. Files over 8 MiB open in the browser."
                    case .failure(let error): notice = error.localizedDescription
                    }
                    recording = nil
                }
                .alert("This world has not been saved", isPresented: $confirmLeave) {
                    Button("Keep this world", role: .cancel) { pendingSource = nil }
                    Button("Leave without saving", role: .destructive) {
                        if let next = pendingSource { host.selectSource(next, discardingUnsaved: true) }
                        pendingSource = nil
                    }
                } message: { Text("Keep this world and use Save recording to retain the current visit. Leaving opens the other world and discards this visit’s unsaved progress.") }
        }
    }
}

private struct PetRecordingDocument: FileDocument {
    static var readableContentTypes: [UTType] { [.json] }
    let data: Data
    init(data: Data) { self.data = data }
    init(configuration: ReadConfiguration) throws {
        guard let data = configuration.file.regularFileContents, data.count <= 64 * 1024 * 1024 else { throw PetCoreError.invalid("Invalid pet recording file.") }
        self.data = data
    }
    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper { FileWrapper(regularFileWithContents: data) }
}
