package codewhale.pet

import app.cash.zipline.EngineApi
import app.cash.zipline.InterruptHandler
import app.cash.zipline.QuickJs
import org.json.JSONArray
import org.json.JSONObject
import kotlin.math.roundToLong
import java.io.OutputStream

data class PetFood(val x: Float, val y: Float, val life: Float)
data class PetScene(
    val timeMs: Double, val state: PetState, val style: Frame, val dots: FloatArray,
    val behaviour: String, val needs: String, val surface: Float, val caustic: Float,
    val food: PetFood?, val peers: Int, val digest: String,
)

/** Confined to one worker. QuickJS runs the committed world, validation and
 * score; Kotlin only projects its state into the existing particle renderer. */
@OptIn(EngineApi::class)
class PetNativeCore(bundle: String, pointsText: String, tape: String = "", saved: String? = null) : AutoCloseable {
    private val js = QuickJs.create()
    private var deadline = Long.MAX_VALUE
    val sim: PetSim
    private var frame: JSONObject
    val timeMs get() = frame.getDouble("timeMs")
    val voices: JSONArray get() = frame.getJSONArray("voices")

    init {
        try {
            js.memoryLimit = 64L * 1024 * 1024
            js.interruptHandler = InterruptHandler { System.nanoTime() > deadline || Thread.currentThread().isInterrupted }
            // The binding's QuickJS predates these two standard ES2022 methods.
            // Only platform shims; the packaged world is byte-identical to Apple/TUI.
            evaluate("""
                if (!Object.hasOwn) Object.defineProperty(Object, 'hasOwn', {value: (o, k) => Object.prototype.hasOwnProperty.call(o, k)});
                if (!Array.prototype.at) Object.defineProperty(Array.prototype, 'at', {value: function(i) { i = Math.trunc(Number(i) || 0); return this[i < 0 ? this.length + i : i]; }});
            """.trimIndent())
            evaluate(bundle)
            val points = pointsText.lineSequence().filter { it.isNotBlank() }.map { row ->
                val p = row.split('\t').map(String::toDouble)
                require(p.size == 2 && p.all { it.isFinite() && it in -1.0..1.0 })
                p[0] to p[1]
            }.toList()
            require(points.size == 980)
            val body = JSONArray(points.map { listOf(it.first, it.second) }).toString()
            evaluate("globalThis.pet = new PetNative(${quote(body)}, ${quote(tape)}); undefined")
            if (saved != null) {
                require(saved.toByteArray(Charsets.UTF_8).size <= MAX_HABITAT_BYTES) { "Habitat exceeds 8 MiB." }
                evaluate("pet.restoreRecording(${quote(saved)})", 10_000)
            }
            frame = JSONObject(string("pet.snapshot()"))
            sim = PetSim(points)
            if (saved != null) {
                val c = JSONObject(string("pet.checkpoint()")).getJSONObject("sim")
                sim.restoreValidated(decodeCheckpoint(c))
                check(petDigest(sim) == frame.getString("digest")) { "Restored particles differ from the shared world." }
            }
        } catch (error: Throwable) {
            js.close()
            throw error
        }
    }

    fun tick(motion: Boolean): PetScene {
        frame = JSONObject(string("pet.step(1 / 30, $motion)"))
        val state = decodeState(frame.getJSONObject("state"))
        val pod = frame.getJSONArray("pod")
        val peers = (0 until pod.length()).map(pod::getJSONObject).filter { it.getBoolean("present") }
        val slots = peers.takeIf { it.size >= 3 }?.map {
            listOf(0, 2, 4, 1, 3, 5)[it.getInt("slot")] to it.getDouble("phase")
        }
        sim.step(1.0 / 30, state, motion, podSlots = slots)
        return scene()
    }

    fun scene(): PetScene {
        val food = frame.optJSONObject("food")?.let {
            PetFood(it.getDouble("x").toFloat(), it.getDouble("y").toFloat(), it.getDouble("life").toFloat())
        }
        val pod = frame.getJSONArray("pod")
        return PetScene(timeMs, decodeState(frame.getJSONObject("state")), sim.frame.copy(),
            FloatArray(sim.p.size * 2) { i -> (if (i % 2 == 0) sim.p[i / 2].x else sim.p[i / 2].y).toFloat() },
            frame.getString("behaviour"), frame.getString("needs"), frame.getDouble("surface").toFloat(),
            frame.getDouble("caustic").toFloat(), food,
            (0 until pod.length()).count { pod.getJSONObject(it).getBoolean("present") }, frame.getString("digest"))
    }

    fun interact(food: Boolean) { evaluate("pet.interact('${if (food) "food" else "attention"}', 0.2, -0.15)") }
    fun recording(): String = string("pet.recording(true)").also {
        require(it.toByteArray(Charsets.UTF_8).size <= MAX_HABITAT_BYTES) { "Habitat exceeds 8 MiB; export the recording." }
    }
    /** Called synchronously on the world worker, so the chunks share one clock.
     * A larger recovery export never materializes one giant string in QuickJS. */
    fun exportRecording(output: OutputStream): Long {
        var size = 0L
        var index = 0
        while (true) {
            val value = evaluate("pet.recordingChunk(${index++})") ?: return size
            val bytes = (value as? String ?: error("Invalid pet export chunk.")).toByteArray(Charsets.UTF_8)
            check(size + bytes.size <= 64L * 1024 * 1024) { "Recording exceeds the 64 MiB export limit. The current world was kept." }
            output.write(bytes); size += bytes.size
        }
    }
    fun pcm(voices: JSONArray, start: Long, length: Int): FloatArray {
        require(start >= 0 && length in 1..24_000 && voices.length() <= 128)
        val channels = JSONArray(string("pet.pcm(${quote(voices.toString())}, $start, $length, 48000)", 500))
        require(channels.length() == 2)
        val left = channels.getJSONArray(0); val right = channels.getJSONArray(1)
        require(left.length() == length && right.length() == length)
        return FloatArray(length * 2) { i ->
            (if (i % 2 == 0) left else right).getDouble(i / 2).toFloat().also {
                require(it.isFinite() && it in -1f..1f) { "Invalid pet audio sample." }
            }
        }
    }
    private fun string(script: String, budgetMs: Long = 5_000) = evaluate(script, budgetMs) as? String
        ?: error("The pet runtime did not return a valid snapshot.")
    private fun evaluate(script: String, budgetMs: Long = 5_000): Any? {
        deadline = System.nanoTime() + budgetMs * 1_000_000
        return try { js.evaluate(script, "pet-host.js") } finally { deadline = Long.MAX_VALUE }
    }
    override fun close() = js.close()

    companion object {
        const val MAX_HABITAT_BYTES = 8 * 1024 * 1024
        private fun quote(text: String): String = JSONObject.quote(text)
        private fun decodeState(s: JSONObject) = PetState(s.getDouble("activity"), s.getDouble("coherence"),
            s.getDouble("attention"), s.getString("channel"), s.getDouble("observed"), s.getDouble("roamX"),
            s.getDouble("roamY"), s.getDouble("flip"), s.getDouble("lit"))
        private fun decodeStyle(f: JSONObject) = Frame(f.getDouble("r"), f.getDouble("g"), f.getDouble("b"),
            f.getDouble("alpha"), f.getBoolean("hollow"), f.getString("channel"), f.getString("arch"), f.getDouble("work"))
        private fun decodeCheckpoint(c: JSONObject): PetParticleCheckpoint {
            fun list(a: JSONArray) = (0 until a.length()).map(a::getDouble)
            fun rows(name: String): List<List<Double>> = c.getJSONArray(name).let { a ->
                (0 until a.length()).map { list(a.getJSONArray(it)) }
            }
            return PetParticleCheckpoint(c.getInt("version"), c.optInt("expressionVersion", 1), rows("body"), rows("particles"),
                c.getDouble("phase"), c.getDouble("clock"), c.getDouble("tear"), c.getInt("previous"), c.getInt("current"),
                list(c.getJSONArray("color")), decodeStyle(c.getJSONObject("frame")))
        }
    }
}

/** Continuously address the shared PCM by simulation sample; retaining a voice
 * never schedules it again. Reopening sound begins at the current clock. */
class PetAudioCursor(timeMs: Double) {
    private var sample = (timeMs * 48).roundToLong()
    private val active = linkedMapOf<String, JSONObject>()
    fun next(core: PetNativeCore): FloatArray? {
        val end = (core.timeMs * 48).roundToLong()
        require(end >= sample && end - sample <= 24_000)
        for (i in 0 until core.voices.length()) {
            val voice = core.voices.getJSONObject(i)
            active[voice.getString("id")] = voice
        }
        active.entries.removeAll { (_, v) -> (v.getDouble("start") + v.getDouble("duration")) * 48_000 <= sample }
        require(active.size <= 128)
        if (end == sample) return null
        return core.pcm(JSONArray(active.values.toList()), sample, (end - sample).toInt()).also { sample = end }
    }
}
