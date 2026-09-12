package codewhale.pet

import android.util.AtomicFile
import java.io.File
import java.io.ByteArrayOutputStream
import java.io.InputStream
import java.io.RandomAccessFile
import java.nio.channels.OverlappingFileLockException
import java.security.MessageDigest

/** Private, bounded, atomic recordings. The revision check also protects a
 * second window/process from silently overwriting a newer habitat. */
class PetHabitatStore(private val directory: File, private val name: String) {
    init { require(name in setOf("wild", "demo", "recording")) }
    private val file = AtomicFile(File(directory, "pet-$name.json"))
    private val lockFile = File(directory, "pet-$name.lock")
    var revision: String? = null; private set

    fun read(): String? = locked {
        revision = currentRevision()
        if (revision == null) null else file.openRead().use { boundedRead(it) }
    }

    fun save(text: String, archive: File? = null, tick: Long = 0) = locked {
        val bytes = text.toByteArray(Charsets.UTF_8)
        require(bytes.size <= PetNativeCore.MAX_HABITAT_BYTES) { "Habitat exceeds 8 MiB." }
        check(currentRevision() == revision) { "Another window changed this habitat. Reopen it before saving." }
        if (archive != null) {
            require(archive.length() <= 64L * 1024 * 1024 && tick >= 0)
            val digest = archive.inputStream().use(::hashInput)
            val target = File(directory, "pet-$name-segment-${"%012d".format(java.util.Locale.ROOT, tick)}-$digest.json")
            if (target.exists()) check(target.inputStream().use(::hashInput) == digest) { "An archived recording was changed." }
            else {
                val atomic = AtomicFile(target)
                val output = atomic.startWrite()
                try { archive.inputStream().use { it.copyTo(output) }; atomic.finishWrite(output) }
                catch (e: Throwable) { atomic.failWrite(output); throw e }
            }
        }
        write(bytes)
        revision = hash(text)
    }

    /** Called only after the explicit Start fresh confirmation. Rename keeps
     * even an oversized/corrupt original without decoding or duplicating it. */
    fun restart(text: String): File? = locked {
        val bytes = text.toByteArray(Charsets.UTF_8)
        require(bytes.size <= PetNativeCore.MAX_HABITAT_BYTES)
        check(currentRevision() == revision) { "Another window changed this habitat. Reopen it before restarting." }
        val backup = if (file.baseFile.exists()) File(file.baseFile.parentFile,
            file.baseFile.nameWithoutExtension + "-recovery-${java.util.UUID.randomUUID()}.json") else null
        if (backup != null) check(file.baseFile.renameTo(backup)) { "Could not preserve the previous habitat." }
        try { write(bytes); revision = hash(text) } catch (e: Throwable) {
            if (!file.baseFile.exists()) backup?.renameTo(file.baseFile)
            throw e
        }
        backup
    }

    fun archives(): List<String> = directory.list()?.filter { validArchive(it) }?.sortedDescending() ?: emptyList()
    fun archive(key: String): File {
        require(validArchive(key)) { "Invalid recording archive." }
        val saved = File(directory, key)
        require(saved.isFile && saved.length() <= 64L * 1024 * 1024) { "This recording is unavailable." }
        val digest = saved.inputStream().use(::hashInput)
        check(key.endsWith("-$digest.json")) { "The archived recording was changed. Its file was kept." }
        return saved
    }
    private fun validArchive(key: String) = key.matches(Regex("pet-$name-segment-[0-9]{12}-[a-f0-9]{64}\\.json"))
    private fun hashInput(input: InputStream): String {
        val digest = MessageDigest.getInstance("SHA-256"); val buffer = ByteArray(8192)
        while (true) { val count = input.read(buffer); if (count < 0) break; digest.update(buffer, 0, count) }
        return digest.digest().joinToString("") { "%02x".format(it) }
    }

    private fun currentRevision(): String? {
        if (!file.baseFile.exists() && !File(file.baseFile.path + ".bak").exists()) return null
        return file.openRead().use { input ->
            val digest = MessageDigest.getInstance("SHA-256"); val buffer = ByteArray(8192)
            while (true) { val n = input.read(buffer); if (n < 0) break; digest.update(buffer, 0, n) }
            digest.digest().joinToString("") { "%02x".format(it) }
        }
    }
    private fun write(bytes: ByteArray) {
        val out = file.startWrite()
        try { out.write(bytes); file.finishWrite(out) } catch (e: Throwable) { file.failWrite(out); throw e }
    }

    private fun <T> locked(block: () -> T): T {
        lockFile.parentFile?.mkdirs()
        return RandomAccessFile(lockFile, "rw").use { handle ->
            val lock = try { handle.channel.tryLock() } catch (_: OverlappingFileLockException) { null }
            check(lock != null) { "Another window is saving this habitat. Try again." }
            lock.use { block() }
        }
    }
    companion object {
        fun boundedRead(input: InputStream): String {
            val out = ByteArrayOutputStream()
            val buffer = ByteArray(8192)
            while (true) {
                val n = input.read(buffer)
                if (n < 0) break
                require(out.size() + n <= PetNativeCore.MAX_HABITAT_BYTES) { "Habitat exceeds 8 MiB." }
                out.write(buffer, 0, n)
            }
            val bytes = out.toByteArray()
            return Charsets.UTF_8.newDecoder().decode(java.nio.ByteBuffer.wrap(bytes)).toString()
        }
        private fun hash(text: String) = MessageDigest.getInstance("SHA-256").digest(text.toByteArray(Charsets.UTF_8))
            .joinToString("") { "%02x".format(it) }
    }
}
