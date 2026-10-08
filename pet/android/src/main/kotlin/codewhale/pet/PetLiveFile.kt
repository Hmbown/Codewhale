package codewhale.pet

import android.content.ContentResolver
import android.net.Uri
import android.os.CancellationSignal
import android.os.ParcelFileDescriptor
import java.nio.ByteBuffer

/** Read only the tail of a user-selected, seekable local document. Reads stay
 * off the world worker; cancellation closes the descriptor and provider request.
 * Packet validation, sequence restarts and freshness belong to the shared core. */
class PetLiveFile(private val resolver: ContentResolver, private val uri: Uri) : AutoCloseable {
    private val cancellation = CancellationSignal()
    @Volatile private var descriptor: ParcelFileDescriptor? = null
    init { require(uri.scheme == "content") { "Choose a local document through the file picker." } }
    fun readTail(): String {
        val opened = resolver.openFileDescriptor(uri, "r", cancellation) ?: error("The local tape could not be opened.")
        descriptor = opened
        try {
            cancellation.throwIfCanceled()
            return ParcelFileDescriptor.AutoCloseInputStream(opened).use { stream ->
                val channel = stream.channel
                val size = channel.size()
                val length = minOf(size, 262_144L).toInt()
                channel.position(size - length)
                val buffer = ByteBuffer.allocate(length)
                while (buffer.hasRemaining() && channel.read(buffer) > 0) cancellation.throwIfCanceled()
                String(buffer.array(), 0, buffer.position(), Charsets.UTF_8)
            }
        } finally { descriptor = null; opened.close() }
    }
    override fun close() {
        cancellation.cancel()
        runCatching { descriptor?.close() }
    }
}
