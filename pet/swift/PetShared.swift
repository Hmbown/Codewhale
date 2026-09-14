import Foundation
import SwiftUI
import Darwin
#if os(macOS)
import AppKit
#else
import UIKit
#endif

public struct PetConnection: Decodable {
    public let version: Int, port: Int, token: String, identity: String
    public func validate() throws {
        guard version == 1, (1...65535).contains(port), token.range(of: "^[a-f0-9]{64}$", options: .regularExpression) != nil,
              UUID(uuidString: identity) != nil else { throw PetCoreError.invalid("Invalid shared pet connection.") }
    }
}
public struct PetAppearance: Decodable {
    public let background: [Double], backgroundTop: [Double], particle: [Double]
    public let eventColors: Bool, brightness: Double, dotScale: Double, glow: Double, environment: Bool
    public var valid: Bool { [background,backgroundTop,particle].allSatisfy { $0.count == 3 && $0.allSatisfy { $0.isFinite && (0...255).contains($0) } } && brightness.isFinite && (0.25...2).contains(brightness) && dotScale.isFinite && (0.65...1.8).contains(dotScale) && glow.isFinite && (0...1).contains(glow) }
    public func color(_ values: [Double]) -> Color {
        guard values.count == 3 else { return Color.black }
        return Color(red: values[0]/255, green: values[1]/255, blue: values[2]/255)
    }
}
public struct PetProjection: Decodable {
    public let points: [[Double]], style: Frame, state: PetState
}
public struct PetActionCue: Decodable {
    public let label: String, tool: String?, observed: Bool, parallel: Int
    public var caption: String { label + (tool.map { " · " + $0 } ?? "") + (parallel > 0 ? " · \(parallel) parallel agents" : "") }
}
public struct PetSharedFrame: Decodable {
    public let version: Int, identity: String, epoch: String, tick: UInt64, cursor: UInt64, source: String, sourceRevision: UInt64
    public let timeMs: Double, behaviour: String, needs: String, digest: String
    public let points: [[Double]], style: Frame, state: PetState, still: PetProjection
    public let appearance: PetAppearance?
    public let activity: PetActionCue?
    public let producerConnected: Bool, storageAvailable: Bool, audioOwner: String?, audioUnavailable: Bool
    public func validate() throws {
        guard version == 1, UUID(uuidString: identity) != nil, UUID(uuidString: epoch) != nil,
              timeMs.isFinite, timeMs >= 0, digest.count == 16, appearance?.valid != false,
              [points, still.points].allSatisfy({ $0.count == 980 && $0.allSatisfy { $0.count == 2 && $0.allSatisfy { $0.isFinite && abs($0) <= 8 } } })
        else { throw PetCoreError.invalid("Invalid shared pet frame.") }
    }
}

/// A view, never a world. Its task polls immutable owner snapshots. Reopening
/// reads the same private descriptor; closing cannot stop the companion.
@MainActor public final class PetSharedHost: ObservableObject {
    @Published public private(set) var frame: PetSharedFrame?
    @Published public private(set) var previous: PetSharedFrame?
    @Published public private(set) var message = "Connecting to the shared pet…"
    @Published public private(set) var sound = false
    @Published public private(set) var changedAt = ProcessInfo.processInfo.systemUptime
    private var connection: PetConnection?
    private var task: Task<Void, Never>?
    private var viewCount = 0
    private let client = UUID().uuidString
    private var sequence: UInt64 = 0
    private var pending: Data?
    private var sending = false
    private var lastLease = 0.0
    private var lastStart = 0.0
    private let session: URLSession = {
        let config = URLSessionConfiguration.ephemeral
        config.timeoutIntervalForRequest = 1.5; config.timeoutIntervalForResource = 2
        config.urlCache = nil
        return URLSession(configuration: config)
    }()
    public init() {}
    public var fresh: Bool { ProcessInfo.processInfo.systemUptime - changedAt < 0.8 }
    private var descriptorURL: URL {
        #if os(macOS)
        return URL(fileURLWithPath: ProcessInfo.processInfo.environment["CODEWHALE_PET_HOME"] ?? NSHomeDirectory() + "/.codewhale/pet-shared").appendingPathComponent("connection.json")
        #else
        return FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0].appendingPathComponent("pet-connection.json")
        #endif
    }
    private func loadConnection() throws {
        let fd = open(descriptorURL.path, O_RDONLY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC)
        guard fd >= 0 else { throw PetCoreError.invalid("Run /pet in Codewhale to start the shared pet.") }
        defer { close(fd) }; var info = stat()
        guard fstat(fd, &info) == 0, info.st_mode & S_IFMT == S_IFREG, info.st_nlink == 1, info.st_size <= 4096 else { throw PetCoreError.invalid("Invalid shared connection file.") }
        var bytes = [UInt8](repeating: 0, count: 4097); let count = Darwin.read(fd, &bytes, bytes.count)
        guard count > 0, count <= 4096 else { throw PetCoreError.invalid("Shared connection could not be read.") }
        let next = try JSONDecoder().decode(PetConnection.self, from: Data(bytes.prefix(count))); try next.validate(); connection = next
    }
    private func startOwnerIfAvailable() {
        #if os(macOS)
        let now = ProcessInfo.processInfo.systemUptime
        guard now - lastStart >= 5 else { return }; lastStart = now
        let binary = ProcessInfo.processInfo.environment["CODEWHALE_PET_EXECUTABLE"].map { URL(fileURLWithPath: $0) }
            ?? Bundle.main.url(forAuxiliaryExecutable: "codewhale-pet-owner")
        if let binary {
            let process = Process(); process.executableURL = binary; process.arguments = ["pet", "serve"]
            process.standardInput = FileHandle.nullDevice; process.standardOutput = FileHandle.nullDevice; process.standardError = FileHandle.nullDevice
            try? process.run()
        }
        #endif
    }
    public func start() {
        guard task == nil else { return }
        task = Task { [weak self] in
            while !Task.isCancelled {
                guard let self else { return }
                do {
                    if self.connection == nil { try self.loadConnection() }
                    let data = try await self.request("/v1/frame")
                    let next = try JSONDecoder().decode(PetSharedFrame.self, from: data); try next.validate()
                    guard next.identity == self.connection?.identity else { throw PetCoreError.invalid("Pet identity changed; reopen its connection.") }
                    if self.frame?.epoch != next.epoch { self.previous = nil; self.changedAt = ProcessInfo.processInfo.systemUptime }
                    else if self.frame?.tick != next.tick { self.previous = self.frame; self.changedAt = ProcessInfo.processInfo.systemUptime }
                    self.frame = next
                    self.message = !self.fresh ? "Owner paused · unobserved" : next.producerConnected ? "Following \(next.source)" : "\(next.source) · telemetry unobserved"
                    if !next.storageAvailable { self.message += " · storage unavailable; previous recording kept" }
                    if next.audioUnavailable { self.sound = false; self.message += " · sound unavailable" }
                    if self.sound && self.fresh && ProcessInfo.processInfo.systemUptime - self.lastLease > 0.5 { await self.setSound(true) }
                    if self.pending != nil { await self.sendPending() }
                } catch {
                    self.message = error.localizedDescription; self.connection = nil; self.sound = false; self.startOwnerIfAvailable()
                }
                try? await Task.sleep(for: .milliseconds(33))
            }
        }
    }
    public func attachView() { viewCount += 1; start() }
    public func detachView() { viewCount = max(0, viewCount - 1); if viewCount == 0 { stop() } }
    public func stop() {
        task?.cancel(); task = nil
        if sound { Task { await setSound(false) } }
    }
    private func request(_ path: String, body: Data? = nil) async throws -> Data {
        guard let connection else { throw PetCoreError.invalid("No shared pet connection.") }
        var request = URLRequest(url: URL(string: "http://127.0.0.1:\(connection.port)\(path)")!)
        request.setValue("Bearer " + connection.token, forHTTPHeaderField: "Authorization")
        if let body { request.httpMethod = "POST"; request.httpBody = body; request.setValue("application/json", forHTTPHeaderField: "Content-Type") }
        let (data, response) = try await session.data(for: request)
        guard data.count <= (path == "/v1/export" ? 64 : 8) * 1024 * 1024, (response as? HTTPURLResponse)?.statusCode == 200 else {
            let error = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
            let message = error?["error"] as? String ?? "Shared pet unavailable."
            if path == "/v1/action", (response as? HTTPURLResponse)?.statusCode == 409, !message.contains("storage") { pending = nil }
            throw PetCoreError.invalid(message)
        }
        return data
    }
    public func interact(food: Bool) {
        guard let frame, fresh, pending == nil else { return }
        pending = try? JSONSerialization.data(withJSONObject: ["identity":frame.identity,"client":client,"seq":sequence + 1,"source_revision":frame.sourceRevision,
            "action":["kind":"interact","food":food,"x":0.2,"y":-0.15] as [String:Any]])
        Task { await sendPending() }
    }
    private func sendPending() async {
        guard let pending, !sending else { return }; sending = true; defer { sending = false }
        do { _ = try await request("/v1/action", body: pending); sequence += 1; self.pending = nil }
        catch { message = error.localizedDescription }
    }
    public func setSound(_ enabled: Bool) async {
        do {
            let data = try await request("/v1/audio", body: JSONSerialization.data(withJSONObject: ["client":client,"enabled":enabled]))
            let response = try JSONSerialization.jsonObject(with: data) as? [String:Any]
            sound = response?["granted"] as? Bool ?? false; lastLease = ProcessInfo.processInfo.systemUptime
            if enabled && !sound { message = "Another view owns sound." }
        } catch { sound = false; message = error.localizedDescription }
    }
    public func openAppearance() {
        guard let connection, let url=URL(string:"http://127.0.0.1:\(connection.port)/#\(connection.token)") else { return }
        #if os(macOS)
        NSWorkspace.shared.open(url)
        #else
        UIApplication.shared.open(url)
        #endif
    }
    public func export() async throws -> Data { try await request("/v1/export") }
}

public struct PetSharedHabitat: View {
    @ObservedObject public var host: PetSharedHost
    public let still: Bool
    public init(host: PetSharedHost, still: Bool) { self.host = host; self.still = still }
    public var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            TimelineView(.animation(minimumInterval: 1.0 / 60, paused: still)) { _ in
                Canvas { ctx, size in
                    let bounds = CGRect(origin: .zero, size: size)
                    let appearance = host.frame?.appearance
                    let colors = appearance.map { [$0.color($0.backgroundTop), $0.color($0.background)] } ?? [Color(red:0.075,green:0.15,blue:0.19),Color(red:0.027,green:0.05,blue:0.07)]
                    ctx.fill(Path(bounds), with: .linearGradient(Gradient(colors:colors), startPoint:.zero, endPoint:CGPoint(x:size.width,y:size.height)))
                    guard let f = host.frame else { return }
                    let state = still ? f.still.state : f.state, style = still ? f.still.style : f.style, points = still ? f.still.points : f.points
                    let lay = petLayout(w:size.width,h:size.height,state:state)
                    let fraction = min(1,max(0,(ProcessInfo.processInfo.systemUptime-host.changedAt)*30))
                    let color = Color(red:style.r/255,green:style.g/255,blue:style.b/255)
                    let t = still ? 0 : f.timeMs / 1000
                    for lane in 0..<(appearance?.environment == false ? 0 : 6) {
                        var path = Path()
                        for x in stride(from:0.0,through:size.width,by:8) {
                            let y=size.height*0.85+sin(x/110+Double(lane)+t*0.18)*8+Double(lane)*5
                            if x==0 {path.move(to:CGPoint(x:x,y:y))} else {path.addLine(to:CGPoint(x:x,y:y))}
                        }
                        ctx.stroke(path,with:.color(.cyan.opacity(0.035)),lineWidth:1)
                    }
                    for (i,q) in points.enumerated() {
                        let before = !still && host.fresh ? host.previous?.points[i] : nil
                        let x = before.map {$0[0]+(q[0]-$0[0])*fraction} ?? q[0], y = before.map {$0[1]+(q[1]-$0[1])*fraction} ?? q[1]
                        let depth=0.65+0.35*Double(i*37%101)/100, radius=max(0.7,lay.dot*0.31)*depth*(appearance?.dotScale ?? 1)
                        let point=CGPoint(x:lay.ox+x*lay.scale*lay.flipX,y:lay.oy+y*lay.scale)
                        let dot=Path(ellipseIn:CGRect(x:point.x-radius,y:point.y-radius,width:radius*2,height:radius*2))
                        if style.hollow || !f.producerConnected || !host.fresh {ctx.stroke(dot,with:.color(color.opacity(style.alpha*depth)),lineWidth:0.7)}
                        else {
                            ctx.fill(dot,with:.color(color.opacity(style.alpha*depth)))
                            let glow=appearance?.glow ?? 0.5
                            if glow > 0 && i % 5 == 0 { let r=radius*(1+glow*5); ctx.fill(Path(ellipseIn:CGRect(x:point.x-r,y:point.y-r,width:r*2,height:r*2)),with:.color(color.opacity(0.055*glow))) }
                        }
                    }
                }
            }.clipShape(RoundedRectangle(cornerRadius:5))
                .accessibilityLabel(host.frame.map {"Shared pet, \($0.style.channel), \($0.style.arch), \($0.behaviour)\($0.state.observed < 0.92 || !$0.producerConnected || !host.fresh ? ", unobserved" : "")"} ?? "Connecting to shared pet")
            if let f=host.frame {
                if let activity=f.activity, activity.observed, host.fresh { Text(activity.caption).font(.callout).accessibilityAddTraits(.updatesFrequently) }
                Text("\(f.style.channel) · \(f.style.arch) · \(f.behaviour)").font(.caption.monospaced())
            }
            Text(host.message).font(.caption).foregroundStyle(.secondary)
            HStack {
                Button("Appearance…"){host.openAppearance()}; Button("Focus"){host.interact(food:false)}; Button("Pulse"){host.interact(food:true)}
                Toggle("Companion sound",isOn:Binding(get:{host.sound},set:{ value in Task {await host.setSound(value)} }))
            }
            if let f=host.frame {Text("\(f.identity) · tick \(f.tick) · \(f.digest)").font(.system(size:9,design:.monospaced)).foregroundStyle(.secondary).textSelection(.enabled)}
        }.onAppear{host.attachView()}.onDisappear{host.detachView()}
    }
}
