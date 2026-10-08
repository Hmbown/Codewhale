package codewhale.pet

import java.net.HttpURLConnection
import java.net.URL
import java.io.File
import java.util.UUID
import android.os.SystemClock
import org.json.JSONObject

/** Local authenticated view. Android reaches the owner through an explicitly
 * established adb reverse/secure loopback tunnel; no LAN listener or world. */
class PetSharedClient(private val descriptor: File) {
    private val client = UUID.randomUUID().toString()
    private var sequence = 0L
    private var pending: JSONObject? = null
    private var frame: JSONObject? = null
    private var changed = 0L
    private var lease = 0L
    var identity = ""; private set
    var granted = false; private set
    var message = "Join the shared pet from More."; private set
    private fun connection(): JSONObject {
        require(descriptor.isFile && descriptor.length() in 1..4096) { "Join shared pet from More and select its connection.json." }
        return JSONObject(descriptor.readText()).also(::validateConnection)
    }
    private fun request(path: String, body: JSONObject? = null): JSONObject {
        val d = connection()
        val http = URL("http://127.0.0.1:${d.getInt("port")}$path").openConnection() as HttpURLConnection
        try {
            http.instanceFollowRedirects = false; http.connectTimeout = 800; http.readTimeout = 1500
            http.setRequestProperty("Authorization", "Bearer ${d.getString("token")}")
            if (body != null) { http.requestMethod = "POST"; http.doOutput = true; http.setRequestProperty("Content-Type", "application/json"); http.outputStream.use { it.write(body.toString().toByteArray()) } }
            val code = http.responseCode
            val bound = (if (path == "/v1/export") 64 else 8) * 1024 * 1024
            val bytes = (if (code == 200) http.inputStream else http.errorStream)?.use { it.readPetBytes(bound + 1) } ?: error("Shared pet unavailable.")
            require(bytes.size <= bound) { "Shared pet response exceeds its bound." }
            val result = JSONObject(String(bytes, Charsets.UTF_8))
            if (code != 200) {
                val error = result.optString("error", "Shared pet unavailable.")
                if (path == "/v1/action" && code == 409 && !error.contains("storage")) pending = null
                error(error)
            }
            return result
        } finally { http.disconnect() }
    }
    fun poll(still: Boolean, sound: Boolean): PetScene {
        val next = request("/v1/frame")
        require(next.getInt("version") == 1 && next.getString("identity") == connection().getString("identity")) { "Shared identity changed; select its connection again." }
        // Validate every rendered field of both poses before this frame can
        // refresh freshness, lease sound or flush a pending action. org.json
        // parses "NaN"/"Infinity" strings through getDouble, so check finiteness.
        val epoch = next.getString("epoch"); UUID.fromString(epoch)
        val tick = next.getLong("tick"); val timeMs = next.getDouble("timeMs")
        val surface = next.getDouble("surface"); val caustic = next.getDouble("caustic")
        require(tick >= 0 && timeMs.isFinite() && timeMs >= 0 && surface.isFinite() && caustic.isFinite()
            && next.getString("digest").length == 16 && next.getString("behaviour") in BEHAVIOURS && next.getString("needs") in NEEDS) { "Invalid shared pet frame." }
        val poses = listOf(next, next.getJSONObject("still")).map { pose ->
            val points = pose.getJSONArray("points"); require(points.length() == 980) { "Invalid shared pet frame." }
            val dots = FloatArray(1960) { i -> points.getJSONArray(i / 2).also { require(it.length() == 2) }.getDouble(i % 2).also { require(it.isFinite() && kotlin.math.abs(it) <= 8) }.toFloat() }
            val state = PetNativeCore.decodeState(pose.getJSONObject("state")); val style = PetNativeCore.decodeStyle(pose.getJSONObject("style"))
            require(validState(state) && validStyle(style)) { "Invalid shared pet frame." }
            Triple(dots, state, style)
        }
        val appearance = next.optJSONObject("appearance")?.let { a ->
            fun color(key: String): List<Int> { val c=a.getJSONArray(key); require(c.length()==3); return (0..2).map { c.getInt(it).also { n -> require(n in 0..255) } } }
            val dotScale = a.getDouble("dotScale"); val glow = a.getDouble("glow"); require(dotScale.isFinite() && glow.isFinite())
            PetAppearance(color("background"), color("backgroundTop"), dotScale.toFloat().coerceIn(.65f,1.8f), glow.toFloat().coerceIn(0f,1f), a.getBoolean("environment"))
        }
        if (frame?.optString("epoch") != epoch || frame?.optLong("tick") != tick) changed = SystemClock.elapsedRealtime()
        frame = next; identity = "${next.getString("identity")} · tick $tick · ${next.getString("digest")}"
        val fresh = SystemClock.elapsedRealtime() - changed < 800
        message = if (fresh && next.getBoolean("producerConnected")) "Following ${next.getString("source")}" else "${next.getString("source")} · telemetry unobserved"
        if (!next.getBoolean("storageAvailable")) message += " · storage unavailable; previous recording kept"
        if (next.getBoolean("audioUnavailable")) granted = false
        if (SystemClock.elapsedRealtime() - lease >= 500) setSound(sound && fresh)
        flush()
        val (dots, state, style) = poses[if (still) 1 else 0]
        if (!fresh || !next.getBoolean("producerConnected")) { style.hollow = true }
        return PetScene(timeMs, state, style, dots, next.getString("behaviour"), next.getString("needs"),
            surface.toFloat(), if (still) 0f else caustic.toFloat(), null, 0, next.getString("digest"), appearance,
            next.optJSONObject("activity")?.takeIf { it.optBoolean("observed") && fresh }?.let { a ->
                a.getString("label").take(96) + (if (a.isNull("tool")) "" else " · " + a.getString("tool").take(96)) +
                    (if (a.optInt("parallel") > 0) " · ${a.getInt("parallel")} parallel agents" else "")
            })
    }
    fun interact(food: Boolean) {
        val f = frame ?: return
        if (pending != null || SystemClock.elapsedRealtime() - changed >= 800) return
        pending = JSONObject().put("identity", f.getString("identity")).put("client", client).put("seq", sequence + 1)
            .put("source_revision", f.getLong("sourceRevision")).put("action", JSONObject().put("kind", "interact").put("food", food).put("x", .2).put("y", -.15))
        flush()
    }
    private fun flush() { pending?.let { request("/v1/action", it); sequence++; pending = null } }
    fun setSound(enabled: Boolean) { granted = request("/v1/audio", JSONObject().put("client", client).put("enabled", enabled)).getBoolean("granted"); lease = SystemClock.elapsedRealtime() }
    fun detach() { if (granted) runCatching { setSound(false) }; granted = false }
    fun export(): ByteArray = request("/v1/export").toString().toByteArray(Charsets.UTF_8)
    companion object {
        private val BEHAVIOURS = setOf("swim", "dive", "roll", "breathe", "drift", "doze", "wake")
        private val NEEDS = setOf("none", "orient", "approach", "call")
        /** The shared core's validatePetState bounds. */
        private fun validState(s: PetState) = listOf(s.activity, s.coherence, s.attention, s.observed, s.lit).all { it.isFinite() && it in 0.0..1.0 }
            && listOf(s.roamX, s.roamY, s.flip).all { it.isFinite() && kotlin.math.abs(it) <= 1.0 } && CHANNELS.any { it.key == s.channel }
        private fun validStyle(f: Frame) = listOf(f.r, f.g, f.b).all { it.isFinite() && it in 0.0..255.0 } && f.alpha.isFinite() && f.alpha in 0.0..1.0
            && f.work.isFinite() && CHANNELS.any { it.key == f.channel }
        fun validateConnection(d: JSONObject) {
            require(d.getInt("version") == 1 && d.getInt("port") in 1..65535 && d.getString("token").matches(Regex("[a-f0-9]{64}"))) { "Invalid shared pet connection." }
            UUID.fromString(d.getString("identity"))
        }
    }
}

internal fun java.io.InputStream.readPetBytes(limit: Int): ByteArray {
    val out = java.io.ByteArrayOutputStream(); val buffer = ByteArray(16384)
    while (out.size() < limit) {
        val count = read(buffer, 0, minOf(buffer.size, limit - out.size()))
        if (count < 0) break
        if (count > 0) out.write(buffer, 0, count)
    }
    return out.toByteArray()
}
