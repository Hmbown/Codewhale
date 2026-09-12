// PetSim.kt — the Codewhale pet core, Kotlin/Android port.
//
// A faithful port of PetSim.ts. Same 980-point body from whale-points.tsv,
// same mulberry32(0xC0FFEE) jitter, same gait field and spring integration,
// same colour/hollow/brightness encoding. Pure Kotlin + java.lang.Math —
// no Android APIs, so it runs on the JVM for tests and in the app for rendering.
//
// Compile-verified offline with the cached Kotlin 2.3.0 JVM compiler.
// android/verify.sh compares all four tapes/modes against TypeScript.

package codewhale.pet

import kotlin.math.*

data class PetState(
    var activity: Double = 0.35,
    var coherence: Double = 0.8,
    var attention: Double = 0.0,
    var channel: String = "reasoning",
    var observed: Double = 1.0,
    var roamX: Double = 0.0,
    var roamY: Double = 0.0,
    var flip: Double = 1.0,
    var lit: Double = 1.0,
)

data class Channel(val key: String, val label: String, val r: Int, val g: Int, val b: Int,
                   val arch: String, val form: String)

val CHANNELS = listOf(
    Channel("reasoning",     "Model / reasoning",   0x73, 0xc9, 0xb5, "gyre",    "gyre · rolling"),
    Channel("tool",          "Tool calls",          0x74, 0xaa, 0xdd, "strike",  "strike · reaching"),
    Channel("memory",        "Memory / RAG",        0xb6, 0xa7, 0x7f, "gyre",    "gyre · scanning"),
    Channel("code",          "Code execution",      0x9b, 0x9e, 0xd7, "strike",  "strike · along the body"),
    Channel("filesystem",    "Filesystem",          0x92, 0xb9, 0xc9, "strike",  "strike · fanning"),
    Channel("network",       "Network / API",       0xd3, 0xac, 0x74, "cross",   "crossing · one way"),
    Channel("browser",       "Browser / computer",  0x9e, 0xa9, 0xdf, "cross",   "crossing · a sweep"),
    Channel("communication", "Agent messages",      0x83, 0xc5, 0xc9, "cross",   "crossing · two ways"),
    Channel("agent",         "Subagent activity",   0xb0, 0x9a, 0xcb, "pod",     "pod · peers"),
    Channel("orchestration", "Orchestration",       0x6c, 0x87, 0x98, "pod",     "pod · hub"),
    Channel("error",         "Errors / exceptions", 0xe7, 0x91, 0x86, "tear",    "torn · irregular"),
    Channel("human",         "Human interaction",   0xc2, 0xb7, 0x87, "address", "decision · junction"),
    Channel("other",         "Unclassified",        0x73, 0x84, 0x92, "drift",   "drifting · unformed"),
)

val CHANNEL_INDEX = CHANNELS.mapIndexed { i, c -> c.key to i }.toMap()

fun archOf(key: String) = when (key) {
    "reasoning", "memory" -> "gyre"
    "tool", "code", "filesystem" -> "strike"
    "network", "communication", "browser" -> "cross"
    "agent", "orchestration" -> "pod"
    "error" -> "tear"
    "human" -> "address"
    else -> "drift"
}

private val UNKNOWN_RGB = doubleArrayOf(115.0, 132.0, 146.0)
private val REST_RGB = doubleArrayOf(122.0, 214.0, 240.0)

private fun lerp(a: Double, b: Double, t: Double) = a + (b - a) * t
private fun clamp(v: Double, lo: Double = 0.0, hi: Double = 1.0) = min(hi, max(lo, v))

/** mulberry32 — the same 32-bit sequence as every other port. */
class Mulberry32(seed: Int) {
    private var a = seed
    fun next(): Double {
        a += 0x6D2B79F5.toInt()
        var t = a
        t = (t xor t.ushr(15)) * (t or 1)
        t = t xor (t + (t xor t.ushr(7)) * (t or 61))
        return (t xor t.ushr(14)).toUInt().toDouble() / 4294967296.0
    }
}

class Particle {
    var x = 0.0; var y = 0.0; var vx = 0.0; var vy = 0.0
    var s = 0.0; var jx = 0.0; var jy = 0.0
    var pod = 0
    var hx = 0.0; var hy = 0.0; var ang = 0.0; var rad = 0.0; var tail = 0.0
    var tx = 0.0; var ty = 0.0
}

data class Frame(
    var r: Double = 122.0, var g: Double = 214.0, var b: Double = 240.0,
    var alpha: Double = 0.3, var hollow: Boolean = false,
    var channel: String = "reasoning", var arch: String = "gyre", var work: Double = 0.0,
)

// Version 2; the same field math as TypeScript, Swift and Rust.
private fun fieldTarget(q: Particle, t: Double, act: Double, att: Double, key: String): Pair<Double, Double>? {
    val u = q.s * 2 - 1; val lane = q.pod - 2.5; val a = q.s * PI * 2
    val flow = t * (0.35 + act * 0.65)
    return when (key) {
        "reasoning" -> {
            val ring = 0.34 + 0.105 * cos(a * 3 + flow + lane * 0.18)
            ring * cos(a * 2 + flow * 0.3) to ring * sin(a * 2 + flow * 0.3) * 0.7 + 0.10 * sin(a * 3 + flow)
        }
        "memory" -> 0.46 * cos(a + lane * 0.1 + flow * 0.25) to lane * 0.082 + 0.052 * sin(a * 2 + flow)
        "code" -> u * 0.57 to lane * 0.066 + 0.12 * sin(u * 7 + flow * 2 + q.pod * PI / 3)
        "filesystem" -> {
            val branch = max(0.0, (u + 0.3) / 1.3)
            u * 0.56 to lane * 0.13 * branch + 0.025 * sin(u * 8 - flow)
        }
        "tool" -> {
            val reach = 0.14 + (u + 1) * 0.20 + 0.04 * sin(flow * 3 - u * 4)
            cos(q.pod * PI / 3) * reach to sin(q.pod * PI / 3) * reach * 0.8 + q.hy * 0.06
        }
        "browser" -> u * 0.56 to lane * 0.083 + 0.035 * sin(u * 5 - flow * 2)
        "network", "communication" -> {
            val direction = if (key == "communication" && q.pod % 2 == 1) -1.0 else 1.0
            val phase = a + flow * direction
            0.54 * cos(phase) to sin(phase) * (0.12 + q.pod * 0.035) + lane * 0.024
        }
        "human" -> {
            val gap = if (u < 0) -0.075 else 0.075
            u * 0.47 + gap to lane * 0.10 * abs(u) + 0.012 * sin(flow + a) * (1 - att)
        }
        else -> null
    }
}

/** Version 1 is retained for saved recordings. */
private fun gaitTarget(q: Particle, t: Double, act: Double, coh: Double,
                       att: Double, key: String, work: Double, expressionVersion: Int = 1): Pair<Double, Double> {
    val omega = lerp(4.6, 5.2 + act * 2.8, work)
    val breath = 1 + sin(t * 1.85) * lerp(0.048, 0.018, work)
    val flex = sin(q.ang * 2.05 + t * omega) * lerp(0.042, 0.016 + act * 0.028, work) * (0.18 + 0.82 * q.tail)
    var px = cos(q.ang + flex) * q.rad * breath
    var py = sin(q.ang + flex) * q.rad * breath
    px += sin(t * 0.33) * lerp(0.030, 0.014, work)
    py += cos(t * 0.21) * lerp(0.018, 0.010, work)
    if (work < 0.02) return px to py

    var gx = px; var gy = py
    when (archOf(key)) {
        "gyre" -> if (key == "memory") {
            val pulse = 1 + sin(t * (2.4 + act * 1.6) - q.rad * 11) * (0.15 + act * 0.10)
            gx *= pulse; gy *= pulse
        } else {
            val roll = sin(t * (1.05 + act * 0.35)) * (0.48 + act * 0.32)
            val c = cos(roll); val sn = sin(roll)
            gx = px * c - py * sn * 0.88
            gy = px * sn * 0.88 + py * c
        }
        "strike" -> if (key == "tool") {
            val rate = 2.7 + act * 2.1
            val lunge = max(0.0, sin(t * rate)).pow(2)
            gx += lunge * 0.11
            if (q.s > 0.60) {
                val reach = max(0.0, sin(t * rate + q.pod * 0.92)).pow(4) * (0.30 + act * 0.24)
                gx += cos(q.ang) * reach
                gy += sin(q.ang) * reach
            }
        } else if (key == "code") {
            val rate = 3.2 + act * 1.8
            val wave = sin(t * rate - q.tail * 7.5)
            val bump = 0.11 + act * 0.08
            gx += cos(q.ang) * wave * bump
            gy += sin(q.ang) * wave * bump * 1.2
            gx += max(0.0, wave) * 0.07
        } else {
            val rate = 2.15 + act * 1.5
            val side = (q.pod % 2) * 2 - 1
            val w = max(0.0, sin(t * rate + q.pod * 0.72)).pow(2)
            gx += w * 0.055
            gy += side * w * (0.17 + act * 0.13)
        }
        "cross" -> if (key == "browser") {
            val band = (t * (0.55 + act * 0.35)) % 1 * 1.28 - 0.64
            val inBand = max(0.0, 1 - abs(q.hy - band) / 0.08)
            gx += inBand * (0.24 + act * 0.10)
            gy += inBand * 0.02
        } else {
            val two = key == "communication"
            if (q.s < if (two) 0.44 else 0.32) {
                val dir = if (two) (if (q.s < 0.22) 1.0 else -1.0) else 1.0
                val u = (t * (0.38 + act * 0.36) + q.s * 5.2) % 1
                val going = if (u < 0.5) u * 2 else 2 - u * 2
                val e = going * going * (3 - 2 * going)
                gx = lerp(q.hx, dir * 0.80, e)
                gy = q.hy * (1 - e * 0.38) + sin(going * PI) * 0.11 * dir
            }
        }
        "pod" -> {
            val n = 6
            val k = q.pod % n
            val hub = key == "orchestration" && k == 0
            val spread = 0.30 + act * 0.11
            val orbit = t * (0.55 + act * 0.28)
            if (hub) { gx = px * 0.70; gy = py * 0.70 }
            else {
                val slots = if (key == "orchestration") n - 1 else n
                val a = (if (key == "orchestration") k - 1 else k) * (PI * 2 / slots) + orbit
                val sc = 0.34
                gx = q.hx * sc + cos(a) * spread * 1.28
                gy = q.hy * sc + sin(a) * spread * 0.80
            }
        }
        "tear" -> {
            val side = if (q.hx + q.hy < 0) -1.0 else 1.0
            gx += side * (0.24 + (1 - coh) * 0.16)
            gy += side * 0.15
            gx += sin(t * 11.4 + q.s * 40) * (0.045 + act * 0.05)
            gy += cos(t * 9.2 + q.s * 31) * (0.040 + act * 0.045)
        }
        "address" -> {
            val face = 0.90 + att * 0.08
            val th = 0.70
            val z = (q.s - 0.5) * 0.42
            var ax = q.hx * cos(th) + z * sin(th)
            var ay = q.hy
            val disc = 0.48 * face
            ax = lerp(ax, cos(q.ang) * min(0.36, q.rad + 0.06) * 0.95, disc)
            ay = lerp(ay, sin(q.ang) * min(0.36, q.rad + 0.06) * 1.08, disc)
            val grow = 1.20 + sin(t * 1.65) * 0.055
            gx = ax * grow; gy = ay * grow
        }
        else -> {
            val mill = 0.13 + (1 - coh) * 0.10
            gx = q.hx * 0.52 + sin(t * 0.72 + q.jx) * mill
            gy = q.hy * 0.52 + cos(t * 0.54 + q.jy) * mill
        }
    }
    if (expressionVersion == 2) fieldTarget(q, t, act, att, key)?.let { gx = it.first; gy = it.second }
    return lerp(px, gx, work) to lerp(py, gy, work)
}

private fun stillT(key: String) = when (key) {
    "reasoning" -> 1.15; "memory" -> 0.42; "tool" -> 0.30; "code" -> 0.18
    "filesystem" -> 0.48; "network" -> 0.72; "browser" -> 0.95
    "communication" -> 0.58; "agent" -> 1.25; "orchestration" -> 0.85
    "error" -> 0.35; "human" -> 0.05; "other" -> 0.90; else -> 0.4
}

class PetSim(points: List<Pair<Double, Double>>, seed: Int = 0xC0FFEE.toInt(), val expressionVersion: Int = 2) {
    val p: List<Particle>
    private var phase = 0.0
    private var clock = 0.0
    private var tear = 0.0
    private var prev: Int
    private val col = REST_RGB.copyOf()
    private var cur: Int
    var frame = Frame(); private set

    init {
        require(expressionVersion == 1 || expressionVersion == 2)
        val rng = Mulberry32(seed)
        p = points.mapIndexed { i, (hx, hy) ->
            Particle().apply {
                this.hx = hx; this.hy = hy
                x = hx; y = hy; tx = hx; ty = hy
                s = rng.next(); jx = rng.next() * 6.283; jy = rng.next() * 6.283
                pod = i % 6
                ang = atan2(hy, hx)
                rad = hypot(hx, hy)
                tail = clamp(((-hx - hy) * 0.5 + 0.22) / 0.62)
            }
        }
        cur = CHANNEL_INDEX["reasoning"]!!
        prev = cur
    }

    /** Advance the sim by dt seconds under `state`. Identical math to PetSim.ts. */
    fun step(dt: Double, state: PetState, motion: Boolean = true, sensitivity: Double = 1.0) {
        fun s(v: Double) = lerp(0.5, v, sensitivity)
        val act = s(state.activity); val coh = s(state.coherence); val att = s(state.attention)
        val seen = s(state.observed)
        phase += dt * (0.18 + act * 0.55) * (if (motion) 1.0 else 0.0)
        if (motion) clock += dt

        CHANNEL_INDEX[state.channel]?.let { cur = it }
        val shown = cur
        val ch = CHANNELS[shown]

        val work = clamp((act - 0.16) / 0.18)
        val wander = lerp(0.32, 1.0, (1 - coh).pow(1.15))

        if (shown != prev) {
            if (shown == CHANNEL_INDEX["error"]) tear = 1.0
            prev = shown
        }
        tear = if (motion) max(0.0, tear - dt * 1.6) else 0.0

        val split = (1 - coh).pow(1.6) * 0.16 + tear * 0.10
        val blur = (1 - coh).pow(1.45) * 0.22 + tear * 0.18
        val pull = if (motion) 2.2 + coh * 5.2 else 18.0
        val tGait = if (motion) clock else stillT(ch.key)

        for (q in p) {
            if (motion) {
                q.jx += dt * (0.40 + act * 1.1)
                q.jy += dt * (0.34 + act * 0.9)
            }
            val (gx, gy) = gaitTarget(q, tGait, act, coh, att, ch.key, work, expressionVersion)
            val podAng = q.pod * 1.047 + phase * 0.22
            val tx = gx + sin(q.jx + q.s * 9) * blur * wander + cos(podAng) * split
            val ty = gy + cos(q.jy + q.s * 7) * blur * wander + sin(podAng) * split * 0.55
            q.tx = tx; q.ty = ty
            if (!motion) { q.x = tx; q.y = ty; q.vx = 0.0; q.vy = 0.0; continue }
            q.vx += (tx - q.x) * pull * dt
            q.vy += (ty - q.y) * pull * dt
            q.vx *= 0.90; q.vy *= 0.90
            val speed = if (motion) 2.6 else 8.0
            q.x += q.vx * dt * speed
            q.y += q.vy * dt * speed
        }

        val want = if (work > 0.35)
            doubleArrayOf(CHANNELS[shown].r.toDouble(), CHANNELS[shown].g.toDouble(), CHANNELS[shown].b.toDouble())
        else REST_RGB
        val k = if (motion) min(1.0, dt * 2.6) else 1.0
        for (c in 0..2) col[c] += (lerp(UNKNOWN_RGB[c], want[c], seen) - col[c]) * k
        val lit = clamp(state.lit)
        val alpha = (0.22 + act * 0.10) * lerp(0.50, 1.0, coh) * lerp(0.55, 1.0, seen) * lerp(0.35, 1.0, lit)
        frame = Frame(col[0], col[1], col[2],
            alpha = min(0.92, alpha * 1.85),
            hollow = seen < 0.92,
            channel = ch.key, arch = ch.arch, work = work)
    }
}

/** Body-space → renderer-space, same as PetSim.ts layout(). */
data class PetLayout(val scale: Double, val flipX: Double, val ox: Double, val oy: Double, val dot: Double)
fun petLayout(w: Double, h: Double, state: PetState): PetLayout {
    val att = state.attention
    return PetLayout(
        scale = min(w * 0.52, h * 0.92) * (1 + att * 0.07),
        flipX = state.flip,
        ox = w / 2 + state.roamX * w * 0.30,
        oy = h / 2 + state.roamY * h * 0.30 + h * att * 0.05,
        dot = max(1.6, min(w, h) * 0.0092) * (1 + att * 0.18))
}
