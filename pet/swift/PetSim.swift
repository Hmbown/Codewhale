// PetSim.swift — the Codewhale pet core, Swift port.
//
// A faithful, dependency-free port of PetSim.ts. Same 980-point body from
// whale-points.tsv, same mulberry32(0xC0FFEE) jitter, same gait field and
// spring integration, same colour/hollow/brightness encoding. Pure
// Foundation — works on iOS, macOS, and Linux Swift alike.

import Foundation

public struct PetState: Codable, Equatable {
    public var activity: Double = 0.35   // how much work 0..1
    public var coherence: Double = 0.8   // converging school vs thrashing
    public var attention: Double = 0     // interaction salience
    public var channel: String = "reasoning"
    public var observed: Double = 1      // instrumentation coverage
    public var roamX: Double = 0         // tank position -1..1
    public var roamY: Double = 0
    public var flip: Double = 1          // 1 faces right, -1 left, 0 edge-on
    public var lit: Double = 1           // sleep dimmer
    public init() {}
}

public struct Channel {
    public let key: String
    public let label: String
    public let rgb: (Double, Double, Double)
    public let arch: String
    public let form: String
}

public let CHANNELS: [Channel] = [
    Channel(key: "reasoning",     label: "Model / reasoning",   rgb: (0x73, 0xc9, 0xb5), arch: "gyre",    form: "gyre · rolling"),
    Channel(key: "tool",          label: "Tool calls",          rgb: (0x74, 0xaa, 0xdd), arch: "strike",  form: "strike · reaching"),
    Channel(key: "memory",        label: "Memory / RAG",        rgb: (0xb6, 0xa7, 0x7f), arch: "gyre",    form: "gyre · scanning"),
    Channel(key: "code",          label: "Code execution",      rgb: (0x9b, 0x9e, 0xd7), arch: "strike",  form: "strike · along the body"),
    Channel(key: "filesystem",    label: "Filesystem",          rgb: (0x92, 0xb9, 0xc9), arch: "strike",  form: "strike · fanning"),
    Channel(key: "network",       label: "Network / API",       rgb: (0xd3, 0xac, 0x74), arch: "cross",   form: "crossing · one way"),
    Channel(key: "browser",       label: "Browser / computer",  rgb: (0x9e, 0xa9, 0xdf), arch: "cross",   form: "crossing · a sweep"),
    Channel(key: "communication", label: "Agent messages",      rgb: (0x83, 0xc5, 0xc9), arch: "cross",   form: "crossing · two ways"),
    Channel(key: "agent",         label: "Subagent activity",   rgb: (0xb0, 0x9a, 0xcb), arch: "pod",     form: "pod · peers"),
    Channel(key: "orchestration", label: "Orchestration",       rgb: (0x6c, 0x87, 0x98), arch: "pod",     form: "pod · hub"),
    Channel(key: "error",         label: "Errors / exceptions", rgb: (0xe7, 0x91, 0x86), arch: "tear",    form: "torn · irregular"),
    Channel(key: "human",         label: "Human interaction",   rgb: (0xc2, 0xb7, 0x87), arch: "address", form: "decision · junction"),
    Channel(key: "other",         label: "Unclassified",        rgb: (0x73, 0x84, 0x92), arch: "drift",   form: "drifting · unformed"),
]

public func channelIndex(_ key: String) -> Int? { CHANNELS.firstIndex { $0.key == key } }

func archOf(_ key: String) -> String {
    switch key {
    case "reasoning", "memory": return "gyre"
    case "tool", "code", "filesystem": return "strike"
    case "network", "communication", "browser": return "cross"
    case "agent", "orchestration": return "pod"
    case "error": return "tear"
    case "human": return "address"
    default: return "drift"
    }
}

let UNKNOWN_RGB = (115.0, 132.0, 146.0)
let REST_RGB = (122.0, 214.0, 240.0)

@inline(__always) func lerp(_ a: Double, _ b: Double, _ t: Double) -> Double { a + (b - a) * t }
@inline(__always) func clamp01(_ v: Double) -> Double { min(1, max(0, v)) }

/// mulberry32 — the same 32-bit sequence as every other port.
public struct Mulberry32 {
    var a: UInt32
    public init(seed: UInt32) { a = seed }
    public mutating func next() -> Double {
        a = a &+ 0x6D2B79F5
        var t = a
        t = (t ^ (t >> 15)) &* (t | 1)
        t = t ^ (t &+ ((t ^ (t >> 7)) &* (t | 61)))
        return Double(t ^ (t >> 14)) / 4294967296.0
    }
}

public struct Particle {
    public var x: Double = 0, y: Double = 0, vx: Double = 0, vy: Double = 0
    public var s: Double = 0, jx: Double = 0, jy: Double = 0
    public var pod: Int = 0
    public var hx: Double = 0, hy: Double = 0, ang: Double = 0, rad: Double = 0, tail: Double = 0
    public var tx: Double = 0, ty: Double = 0
    public init() {}
}

public struct Frame: Codable {
    public var r: Double = 122, g: Double = 214, b: Double = 240
    public var alpha: Double = 0.3
    public var hollow: Bool = false
    public var channel: String = "reasoning"
    public var arch: String = "gyre"
    public var work: Double = 0
    public init() {}
    public init(r: Double, g: Double, b: Double, alpha: Double, hollow: Bool,
                channel: String, arch: String, work: Double) {
        self.r = r; self.g = g; self.b = b; self.alpha = alpha
        self.hollow = hollow; self.channel = channel; self.arch = arch; self.work = work
    }
}

/// The canonical JavaScript checkpoint is validated before this projection is
/// decoded. Swift restores its existing particle renderer from that same state.
struct PetParticleCheckpoint: Decodable {
    let version: Int
    let expressionVersion: Int?
    let body: [[Double]], particles: [[Double]]
    let phase: Double, clock: Double, tear: Double
    let previous: Int, current: Int, color: [Double]
    let frame: Frame
}

// Version 2 expresses work as fields while preserving seeded particle identity.
func fieldTarget(_ q: Particle, _ t: Double, _ act: Double, _ att: Double, _ key: String) -> (Double, Double)? {
    let u = q.s * 2 - 1, lane = Double(q.pod) - 2.5, a = q.s * Double.pi * 2
    let flow = t * (0.35 + act * 0.65)
    switch key {
    case "reasoning":
        let ring = 0.34 + 0.105 * cos(a * 3 + flow + lane * 0.18)
        return (ring * cos(a * 2 + flow * 0.3), ring * sin(a * 2 + flow * 0.3) * 0.7 + 0.10 * sin(a * 3 + flow))
    case "memory": return (0.46 * cos(a + lane * 0.1 + flow * 0.25), lane * 0.082 + 0.052 * sin(a * 2 + flow))
    case "code": return (u * 0.57, lane * 0.066 + 0.12 * sin(u * 7 + flow * 2 + Double(q.pod) * Double.pi / 3))
    case "filesystem":
        let branch = max(0, (u + 0.3) / 1.3)
        return (u * 0.56, lane * 0.13 * branch + 0.025 * sin(u * 8 - flow))
    case "tool":
        let reach = 0.14 + (u + 1) * 0.20 + 0.04 * sin(flow * 3 - u * 4)
        return (cos(Double(q.pod) * Double.pi / 3) * reach, sin(Double(q.pod) * Double.pi / 3) * reach * 0.8 + q.hy * 0.06)
    case "browser": return (u * 0.56, lane * 0.083 + 0.035 * sin(u * 5 - flow * 2))
    case "network", "communication":
        let direction = key == "communication" && q.pod % 2 == 1 ? -1.0 : 1.0
        let phase = a + flow * direction
        return (0.54 * cos(phase), sin(phase) * (0.12 + Double(q.pod) * 0.035) + lane * 0.024)
    case "human":
        let gap = u < 0 ? -0.075 : 0.075
        return (u * 0.47 + gap, lane * 0.10 * abs(u) + 0.012 * sin(flow + a) * (1 - att))
    default: return nil
    }
}

/// Version 1 is retained for saved recordings.
/// Ported line-for-line from PetSim.ts gaitTarget().
func gaitTarget(_ q: Particle, _ t: Double, _ act: Double, _ coh: Double,
                _ att: Double, _ key: String, _ work: Double, _ podSlots: [(Int, Double)]? = nil, _ expressionVersion: Int = 1) -> (Double, Double) {
    let omega = lerp(4.6, 5.2 + act * 2.8, work)
    let breath = 1 + sin(t * 1.85) * lerp(0.048, 0.018, work)
    let flex = sin(q.ang * 2.05 + t * omega) * lerp(0.042, 0.016 + act * 0.028, work) * (0.18 + 0.82 * q.tail)
    var px = cos(q.ang + flex) * q.rad * breath
    var py = sin(q.ang + flex) * q.rad * breath
    px += sin(t * 0.33) * lerp(0.030, 0.014, work)
    py += cos(t * 0.21) * lerp(0.018, 0.010, work)
    if work < 0.02 { return (px, py) }

    var gx = px, gy = py
    switch archOf(key) {
    case "gyre":
        if key == "memory" {
            let pulse = 1 + sin(t * (2.4 + act * 1.6) - q.rad * 11) * (0.15 + act * 0.10)
            gx *= pulse; gy *= pulse
        } else {
            let roll = sin(t * (1.05 + act * 0.35)) * (0.48 + act * 0.32)
            let c = cos(roll), sn = sin(roll)
            gx = px * c - py * sn * 0.88
            gy = px * sn * 0.88 + py * c
        }
    case "strike":
        if key == "tool" {
            let rate = 2.7 + act * 2.1
            let lunge = pow(max(0, sin(t * rate)), 2)
            gx += lunge * 0.11
            if q.s > 0.60 {
                let reach = pow(max(0, sin(t * rate + Double(q.pod) * 0.92)), 4) * (0.30 + act * 0.24)
                gx += cos(q.ang) * reach
                gy += sin(q.ang) * reach
            }
        } else if key == "code" {
            let rate = 3.2 + act * 1.8
            let wave = sin(t * rate - q.tail * 7.5)
            let bump = 0.11 + act * 0.08
            gx += cos(q.ang) * wave * bump
            gy += sin(q.ang) * wave * bump * 1.2
            gx += max(0, wave) * 0.07
        } else {
            let rate = 2.15 + act * 1.5
            let side = Double(q.pod % 2) * 2 - 1
            let w = pow(max(0, sin(t * rate + Double(q.pod) * 0.72)), 2)
            gx += w * 0.055
            gy += side * w * (0.17 + act * 0.13)
        }
    case "cross":
        if key == "browser" {
            let band = (t * (0.55 + act * 0.35)).truncatingRemainder(dividingBy: 1) * 1.28 - 0.64
            let inBand = max(0, 1 - abs(q.hy - band) / 0.08)
            gx += inBand * (0.24 + act * 0.10)
            gy += inBand * 0.02
        } else {
            let two = key == "communication"
            let courier = q.s < (two ? 0.44 : 0.32)
            if courier {
                let dir: Double = two ? (q.s < 0.22 ? 1 : -1) : 1
                let u = (t * (0.38 + act * 0.36) + q.s * 5.2).truncatingRemainder(dividingBy: 1)
                let going = u < 0.5 ? u * 2 : 2 - u * 2
                let e = going * going * (3 - 2 * going)
                gx = lerp(q.hx, dir * 0.80, e)
                gy = q.hy * (1 - e * 0.38) + sin(going * .pi) * 0.11 * dir
            }
        }
    case "pod":
        let n = 6
        let member = podSlots.flatMap { $0.isEmpty ? nil : $0[q.pod % $0.count] }
        let k = member?.0 ?? q.pod % n
        let hub = key == "orchestration" && k == 0
        let spread = 0.30 + act * 0.11
        let orbit = t * (0.55 + act * 0.28)
        if hub {
            gx = px * 0.70; gy = py * 0.70
        } else {
            let slots = key == "orchestration" ? n - 1 : n
            let a = Double(key == "orchestration" ? k - 1 : k) * (.pi * 2 / Double(slots)) + orbit + (member.map { $0.1 * 0.04 } ?? 0)
            let sc = 0.34
            gx = q.hx * sc + cos(a) * spread * 1.28
            gy = q.hy * sc + sin(a) * spread * 0.80
        }
    case "tear":
        let side: Double = q.hx + q.hy < 0 ? -1 : 1
        gx += side * (0.24 + (1 - coh) * 0.16)
        gy += side * 0.15
        gx += sin(t * 11.4 + q.s * 40) * (0.045 + act * 0.05)
        gy += cos(t * 9.2 + q.s * 31) * (0.040 + act * 0.045)
    case "address":
        let face = 0.90 + att * 0.08
        let th = 0.70
        let z = (q.s - 0.5) * 0.42
        var ax = q.hx * cos(th) + z * sin(th)
        var ay = q.hy
        let disc = 0.48 * face
        ax = lerp(ax, cos(q.ang) * min(0.36, q.rad + 0.06) * 0.95, disc)
        ay = lerp(ay, sin(q.ang) * min(0.36, q.rad + 0.06) * 1.08, disc)
        let grow = 1.20 + sin(t * 1.65) * 0.055
        gx = ax * grow; gy = ay * grow
    default:
        let mill = 0.13 + (1 - coh) * 0.10
        gx = q.hx * 0.52 + sin(t * 0.72 + q.jx) * mill
        gy = q.hy * 0.52 + cos(t * 0.54 + q.jy) * mill
    }
    if expressionVersion == 2, let field = fieldTarget(q, t, act, att, key) { gx = field.0; gy = field.1 }
    return (lerp(px, gx, work), lerp(py, gy, work))
}

func stillT(_ key: String) -> Double {
    switch key {
    case "reasoning": return 1.15; case "memory": return 0.42; case "tool": return 0.30
    case "code": return 0.18; case "filesystem": return 0.48; case "network": return 0.72
    case "browser": return 0.95; case "communication": return 0.58; case "agent": return 1.25
    case "orchestration": return 0.85; case "error": return 0.35; case "human": return 0.05
    case "other": return 0.90; default: return 0.4
    }
}

public final class PetSim {
    public private(set) var p: [Particle]
    public private(set) var expressionVersion: Int
    var phase = 0.0
    var clock = 0.0
    var tear = 0.0
    var prev: Int
    var col = REST_RGB
    var cur: Int
    public private(set) var frame = Frame()

    public init(points: [(Double, Double)], seed: UInt32 = 0xC0FFEE, expressionVersion: Int = 2) {
        precondition(expressionVersion == 1 || expressionVersion == 2)
        self.expressionVersion = expressionVersion
        var rng = Mulberry32(seed: seed)
        p = points.enumerated().map { (i, pt) in
            var q = Particle()
            q.hx = pt.0; q.hy = pt.1
            q.x = pt.0; q.y = pt.1; q.tx = pt.0; q.ty = pt.1
            q.s = rng.next(); q.jx = rng.next() * 6.283; q.jy = rng.next() * 6.283
            q.pod = i % 6
            q.ang = atan2(q.hy, q.hx)
            q.rad = (q.hx * q.hx + q.hy * q.hy).squareRoot()
            q.tail = clamp01(((-q.hx - q.hy) * 0.5 + 0.22) / 0.62)
            return q
        }
        cur = channelIndex("reasoning")!
        prev = cur
    }

    func restoreValidated(_ checkpoint: PetParticleCheckpoint) throws {
        // Array and identity checks also protect this native boundary if its
        // caller changes. Mutation starts only after the complete shape passes.
        guard checkpoint.version == 1, [1, 2].contains(checkpoint.expressionVersion ?? 1), checkpoint.body.count == p.count,
              checkpoint.particles.count == p.count, checkpoint.color.count == 3,
              CHANNELS.indices.contains(checkpoint.previous), CHANNELS.indices.contains(checkpoint.current),
              checkpoint.body.enumerated().allSatisfy({ i, v in
                  v.count == 3 && v[0] == p[i].hx && v[1] == p[i].hy && v[2] == p[i].s
              }), checkpoint.particles.allSatisfy({ $0.count == 8 && $0.allSatisfy(\.isFinite) })
        else { throw NSError(domain: "CodewhalePet", code: 1, userInfo: [NSLocalizedDescriptionKey: "The particle checkpoint does not match this whale."]) }
        expressionVersion = checkpoint.expressionVersion ?? 1
        phase = checkpoint.phase; clock = checkpoint.clock; tear = checkpoint.tear
        prev = checkpoint.previous; cur = checkpoint.current
        col = (checkpoint.color[0], checkpoint.color[1], checkpoint.color[2]); frame = checkpoint.frame
        for i in p.indices {
            let v = checkpoint.particles[i]
            p[i].x = v[0]; p[i].y = v[1]; p[i].vx = v[2]; p[i].vy = v[3]
            p[i].jx = v[4]; p[i].jy = v[5]; p[i].tx = v[6]; p[i].ty = v[7]
        }
    }

    /// Advance the sim by dt seconds under `state`. Identical math to PetSim.ts.
    public func step(dt: Double, state: PetState, motion: Bool = true, sensitivity: Double = 1, podSlots: [(Int, Double)]? = nil) {
        let s: (Double) -> Double = { lerp(0.5, $0, sensitivity) }
        let act = s(state.activity), coh = s(state.coherence), att = s(state.attention)
        let seen = s(state.observed)
        let mot = motion ? 1.0 : 0.0
        phase += dt * (0.18 + act * 0.55) * mot
        if motion { clock += dt }

        if let i = channelIndex(state.channel) { cur = i }
        let shown = cur
        let ch = CHANNELS[shown]

        let work = clamp01((act - 0.16) / 0.18)
        let wander = lerp(0.32, 1, pow(1 - coh, 1.15))

        if shown != prev {
            if shown == channelIndex("error")! { tear = 1 }
            prev = shown
        }
        tear = motion ? max(0, tear - dt * 1.6) : 0

        let split = pow(1 - coh, 1.6) * 0.16 + tear * 0.10
        let blur = pow(1 - coh, 1.45) * 0.22 + tear * 0.18
        let pull = motion ? (2.2 + coh * 5.2) : 18.0
        let tGait = motion ? clock : stillT(ch.key)

        for i in p.indices {
            if motion {
                p[i].jx += dt * (0.40 + act * 1.1)
                p[i].jy += dt * (0.34 + act * 0.9)
            }
            let (gx, gy) = gaitTarget(p[i], tGait, act, coh, att, ch.key, work, podSlots, expressionVersion)
            let podAng = Double(p[i].pod) * 1.047 + phase * 0.22
            let tx = gx + sin(p[i].jx + p[i].s * 9) * blur * wander + cos(podAng) * split
            let ty = gy + cos(p[i].jy + p[i].s * 7) * blur * wander + sin(podAng) * split * 0.55
            p[i].tx = tx; p[i].ty = ty
            if !motion { p[i].x = tx; p[i].y = ty; p[i].vx = 0; p[i].vy = 0; continue }
            p[i].vx += (tx - p[i].x) * pull * dt
            p[i].vy += (ty - p[i].y) * pull * dt
            p[i].vx *= 0.90; p[i].vy *= 0.90
            let speed = motion ? 2.6 : 8.0
            p[i].x += p[i].vx * dt * speed
            p[i].y += p[i].vy * dt * speed
        }

        let want = work > 0.35 ? CHANNELS[shown].rgb : REST_RGB
        let k = motion ? min(1, dt * 2.6) : 1
        col = (col.0 + (lerp(UNKNOWN_RGB.0, want.0, seen) - col.0) * k,
               col.1 + (lerp(UNKNOWN_RGB.1, want.1, seen) - col.1) * k,
               col.2 + (lerp(UNKNOWN_RGB.2, want.2, seen) - col.2) * k)
        let lit = clamp01(state.lit)
        let alpha = (0.22 + act * 0.10) * lerp(0.50, 1, coh) * lerp(0.55, 1, seen) * lerp(0.35, 1, lit)
        frame = Frame(r: col.0, g: col.1, b: col.2,
                      alpha: min(0.92, alpha * 1.85),
                      hollow: seen < 0.92,
                      channel: ch.key, arch: ch.arch, work: work)
    }
}

/// Body-space → renderer-space, same as PetSim.ts layout().
public struct PetLayout {
    public let scale: Double, flipX: Double, ox: Double, oy: Double, dot: Double
}
public func petLayout(w: Double, h: Double, state: PetState) -> PetLayout {
    let att = state.attention
    let scale = min(w * 0.52, h * 0.92) * (1 + att * 0.07)
    return PetLayout(
        scale: scale, flipX: state.flip,
        ox: w / 2 + state.roamX * w * 0.30,
        oy: h / 2 + state.roamY * h * 0.30 + h * att * 0.05,
        dot: max(1.6, min(w, h) * 0.0092) * (1 + att * 0.18))
}

/// Conformance digest — the same 64×32 quantization + FNV-1a as every port.
public func petDigest(_ sim: PetSim) -> String {
    let W = 64, H = 32
    var grid = [UInt8](repeating: 0, count: W * H)
    for q in sim.p {
        let cx = Int(((q.x + 0.66) / 1.32 * Double(W)).rounded(.down))
        let cy = Int(((q.y + 0.66) / 1.32 * Double(H)).rounded(.down))
        if cx >= 0 && cx < W && cy >= 0 && cy < H {
            let i = cy * W + cx
            grid[i] = grid[i] == 255 ? 255 : grid[i] + 1
        }
    }
    var h: UInt64 = 0xcbf29ce484222325
    func mix(_ b: UInt64) { h ^= b & 0xff; h = h &* 0x100000001b3 }
    for v in grid { mix(UInt64(v)) }
    mix(UInt64(sim.frame.r.rounded()))
    mix(UInt64(sim.frame.g.rounded()))
    mix(UInt64(sim.frame.b.rounded()))
    mix(UInt64((sim.frame.alpha * 255).rounded()))
    mix(sim.frame.hollow ? 1 : 0)
    return String(format: "%016llx", h)
}
