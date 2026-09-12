package codewhale.pet

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioTrack
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicReference
import kotlin.concurrent.thread
import kotlin.math.max

/** One optional output device. Slow audio drops a bounded packet, never blocks
 * the world worker, and never builds a backlog to play after a pause. */
class PetAudioOutput(context: Context, onFocusLost: () -> Unit) : AutoCloseable {
    private data class Packet(val samples: FloatArray, val createdMs: Long = SystemClock.elapsedRealtime())
    private val pending = ArrayBlockingQueue<Packet>(4)
    private val running = AtomicBoolean(true)
    private val closed = AtomicBoolean(false)
    private val failure = AtomicReference<String?>(null)
    private val manager = context.getSystemService(AudioManager::class.java)
    private val attributes = AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_MEDIA)
        .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC).build()
    private val focus = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
        .setAudioAttributes(attributes).setOnAudioFocusChangeListener({ change ->
            if (change != AudioManager.AUDIOFOCUS_GAIN) { close(); onFocusLost() }
        }, Handler(Looper.getMainLooper())).build()
    private lateinit var writer: Thread

    init {
        check(manager.requestAudioFocus(focus) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED) { "Another app is using audio. Try Sound again." }
        writer = thread(name = "codewhale-pet-audio", isDaemon = true) {
            var track: AudioTrack? = null
            try {
                val minimum = AudioTrack.getMinBufferSize(48_000, AudioFormat.CHANNEL_OUT_STEREO, AudioFormat.ENCODING_PCM_FLOAT)
                check(minimum > 0) { "Stereo float audio is unavailable." }
                track = AudioTrack.Builder().setAudioAttributes(attributes).setAudioFormat(AudioFormat.Builder()
                    .setSampleRate(48_000).setEncoding(AudioFormat.ENCODING_PCM_FLOAT)
                    .setChannelMask(AudioFormat.CHANNEL_OUT_STEREO).build())
                    .setTransferMode(AudioTrack.MODE_STREAM).setBufferSizeInBytes(max(minimum, 4_800 * 8)).build()
                check(track.state == AudioTrack.STATE_INITIALIZED) { "Audio output could not start." }
                track.play()
                while (running.get()) {
                    val packet = pending.poll(100, TimeUnit.MILLISECONDS) ?: continue
                    var offset = 0
                    while (running.get() && offset < packet.samples.size && SystemClock.elapsedRealtime() - packet.createdMs <= 500) {
                        val n = track.write(packet.samples, offset, packet.samples.size - offset, AudioTrack.WRITE_NON_BLOCKING)
                        check(n >= 0) { "Audio output stopped ($n)." }
                        offset += n
                        if (n == 0) Thread.sleep(4)
                    }
                }
            } catch (_: InterruptedException) {
                // Closing wakes a waiting writer; the device is always released below.
            } catch (e: Exception) {
                failure.set(e.message ?: "Audio output stopped.")
            } finally {
                running.set(false); pending.clear()
                track?.let { runCatching { it.pause(); it.flush() }; it.release() }
            }
        }
    }

    fun offer(samples: FloatArray) {
        failure.get()?.let { error(it) }
        check(running.get()) { "Audio output stopped." }
        require(samples.size in 2..48_000 && samples.size % 2 == 0 && samples.all { it.isFinite() && it in -1f..1f })
        pending.offer(Packet(samples))
    }
    override fun close() {
        if (closed.getAndSet(true)) return
        running.set(false); pending.clear()
        // This may also be called by the focus listener during construction.
        if (this::writer.isInitialized) writer.interrupt()
        manager.abandonAudioFocusRequest(focus)
    }
}
