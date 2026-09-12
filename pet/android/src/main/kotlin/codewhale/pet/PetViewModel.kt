package codewhale.pet

import android.app.Application
import android.content.Intent
import android.net.Uri
import android.os.SystemClock
import java.io.File
import org.json.JSONObject
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicReference
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.withContext

enum class PetMode(val key: String, val label: String, val detail: String) {
    WILD("wild", "Wild", "Simulated creature · no session attached"),
    DEMO("demo", "Event demo", "Synthetic telemetry"),
    RECORDING("recording", "Recording", "Saved telemetry · replay"),
    LIVE("live", "Live", "Following a local telemetry file"),
}
data class PetUiState(val scene: PetScene? = null, val mode: PetMode = PetMode.WILD,
    val paused: Boolean = false, val sound: Boolean = false, val still: Boolean = false,
    val systemStill: Boolean = false, val message: String? = null, val savedAtMs: Double? = null,
    val archives: List<String> = emptyList(), val canExportRecovery: Boolean = false, val running: Boolean = false,
    val canFollowLive: Boolean = false)

/** A single actor owns core, audio cursor and save revision. Compose receives
 * immutable projections and never reads a particle while it is being stepped. */
class PetViewModel(application: Application) : AndroidViewModel(application) {
    private sealed interface Command {
        data class Mode(val mode: PetMode) : Command
        data class Interact(val food: Boolean) : Command
        data class Import(val uri: Uri) : Command
        data class Follow(val uri: Uri) : Command
        data class Export(val uri: Uri) : Command
        data class ExportRecovery(val uri: Uri) : Command
        data class ExportArchive(val uri: Uri, val name: String) : Command
        data object Restart : Command
        data object Reload : Command
    }
    private val prefs = application.getSharedPreferences("pet", 0)
    private var liveUri = prefs.getString("live-uri", null)?.let(Uri::parse)?.takeIf { it.scheme == "content" }
    private val initialMode = PetMode.entries.firstOrNull { it.key == prefs.getString("mode", "wild") } ?: PetMode.WILD
    private val mutable = MutableStateFlow(PetUiState(mode = initialMode, still = prefs.getBoolean("still", false), canFollowLive = liveUri != null))
    val ui = mutable.asStateFlow()
    private val commands = Channel<Command>(64)
    private val worker = Executors.newSingleThreadExecutor { Thread(it, "codewhale-pet-world") }.asCoroutineDispatcher()
    @Volatile private var active = false
    @Volatile private var paused = false
    @Volatile private var sound = false
    @Volatile private var still = prefs.getBoolean("still", false)
    @Volatile private var systemStill = false
    private var core: PetNativeCore? = null
    private var store: PetHabitatStore? = null
    private var saveBlocked = false
    private var mode = initialMode
    private var output: PetAudioOutput? = null
    private var cursor: PetAudioCursor? = null
    private var lastSave = 0L
    private data class LiveSample(val reader: PetLiveFile, val text: String?, val receivedAt: Long)
    private val pendingLive = AtomicReference<LiveSample?>()
    private var liveReader: PetLiveFile? = null
    private var liveTask: Job? = null
    private val assets = application.assets
    private val bundle by lazy { assets.open("pet-native.js").bufferedReader().use { it.readText() } }
    private val points by lazy { assets.open("whale-points.tsv").bufferedReader().use { it.readText() } }
    private fun tape(mode: PetMode) = if (mode == PetMode.DEMO) assets.open("demo.jsonl").bufferedReader().use { it.readText() } else ""

    init {
        viewModelScope.launch {
            try {
                withContext(worker) {
                    try { open(initialMode) } catch (e: Exception) { setPaused(true); message(e) }
                }
                var wasPlaying = false
                while (isActive) {
                    val began = SystemClock.elapsedRealtime()
                    try {
                        withContext(worker) {
                            while (true) {
                                val command = commands.tryReceive().getOrNull() ?: break
                                try { handle(command) } catch (e: Exception) { message(e) }
                            }
                            val framePaused = paused
                            val frameStill = still
                            val frameSystemStill = systemStill
                            val playing = active && !framePaused
                            if (!playing) {
                                stopAudio(); stopLive()
                                if (wasPlaying) save()
                                // Acknowledge Pause only after the in-flight frame,
                                // output and save have settled on their owning worker.
                                mutable.update { it.copy(paused = framePaused, running = false,
                                    still = frameStill, systemStill = frameSystemStill) }
                            } else {
                                val pet = checkNotNull(core)
                                if (mode == PetMode.LIVE) { startLive(); acceptLive(pet, began) }
                                if (!sound) stopAudio()
                                if (sound && output == null) {
                                    try {
                                        output = PetAudioOutput(application) { setSound(false) }
                                        cursor = PetAudioCursor(pet.timeMs)
                                    } catch (e: Exception) { audioFailed(e) }
                                }
                                val scene = pet.tick(!frameStill && !frameSystemStill)
                                if (!active || paused || !sound) stopAudio()
                                if (output != null) {
                                    try { cursor?.next(pet)?.let { output?.offer(it) } }
                                    catch (e: Exception) { audioFailed(e) }
                                }
                                mutable.update { it.copy(scene = scene, paused = false, running = true,
                                    still = frameStill, systemStill = frameSystemStill) }
                                if (began - lastSave >= 5_000) save()
                            }
                            wasPlaying = playing
                        }
                    } catch (e: CancellationException) { throw e }
                    catch (e: Exception) {
                        setPaused(true); setSound(false)
                        withContext(worker) { stopAudio() }
                        message(e)
                    }
                    delay(if (active && !paused) maxOf(1, 33 - (SystemClock.elapsedRealtime() - began)) else 80)
                }
            } catch (e: CancellationException) { throw e }
            catch (e: Exception) { message(e) }
            finally {
                withContext(NonCancellable + worker) {
                    stopAudio(); stopLive(); runCatching { save() }; core?.close(); core = null
                }
                commands.close(); worker.close()
            }
        }
    }

    fun setActive(value: Boolean) { active = value }
    fun setPaused(value: Boolean) { paused = value }
    fun setSound(value: Boolean) { sound = value; mutable.update { it.copy(sound = value) } }
    fun setStill(value: Boolean) { still = value; prefs.edit().putBoolean("still", value).apply() }
    fun setSystemStill(value: Boolean) { systemStill = value }
    fun mode(value: PetMode) = enqueue(Command.Mode(value))
    fun interact(food: Boolean) = enqueue(Command.Interact(food))
    fun importRecording(uri: Uri) = enqueue(Command.Import(uri))
    fun followLocalTape(uri: Uri) = enqueue(Command.Follow(uri))
    fun exportRecording(uri: Uri) = enqueue(Command.Export(uri))
    fun exportRecovery(uri: Uri) = enqueue(Command.ExportRecovery(uri))
    fun exportArchive(uri: Uri, name: String) = enqueue(Command.ExportArchive(uri, name))
    fun restart() = enqueue(Command.Restart)
    fun reload() = enqueue(Command.Reload)
    fun dismissMessage() { mutable.update { it.copy(message = null) } }
    private fun enqueue(command: Command) {
        if (!commands.trySend(command).isSuccess) mutable.update { it.copy(message = "The habitat is busy. Try again.") }
    }
    private fun message(e: Exception) { mutable.update { it.copy(message = e.message?.take(400) ?: "The habitat could not continue.") } }
    private fun stopAudio() { output?.close(); output = null; cursor = null }
    private fun audioFailed(e: Exception) { stopAudio(); setSound(false); mutable.update { it.copy(message = "Sound paused: ${e.message ?: "output unavailable"}") } }
    private fun stopLive() {
        liveTask?.cancel(); liveTask = null
        liveReader?.close(); liveReader = null; pendingLive.set(null)
    }
    private fun startLive() {
        if (liveReader != null) return
        val uri = liveUri ?: return
        checkNotNull(core).resumeLiveInput()
        val reader = PetLiveFile(getApplication<Application>().contentResolver, uri)
        liveReader = reader
        liveTask = viewModelScope.launch(Dispatchers.IO) {
            while (isActive) {
                val text = try { reader.readTail() } catch (e: Exception) { if (!isActive) break; null }
                if (isActive) pendingLive.set(LiveSample(reader, text, SystemClock.elapsedRealtime()))
                delay(400)
            }
        }
    }
    private fun acceptLive(pet: PetNativeCore, now: Long) {
        val sample = pendingLive.getAndSet(null) ?: return
        if (sample.reader !== liveReader) return
        try {
            if (now - sample.receivedAt > 800 || sample.text == null) {
                pet.acceptLiveTail("")
                mutable.update { it.copy(message = "Local tape unavailable or delayed. Use Follow local tape to choose a local, seekable file.") }
            } else if (pet.acceptLiveTail(sample.text)) {
                mutable.update { it.copy(message = "Following local telemetry. Missing input stays unobserved.") }
            }
        } catch (e: Exception) {
            runCatching { pet.acceptLiveTail("") }
            mutable.update { it.copy(message = "Invalid local telemetry · unobserved. The current world is kept.") }
        }
    }
    private fun save(): Boolean {
        lastSave = SystemClock.elapsedRealtime()
        if (saveBlocked) return false
        val pet = core ?: return true
        try {
            val files = checkNotNull(store) { "Habitat storage is unavailable." }
            val next = pet.prepareSegment()
            if (next != null) {
                val staged = File.createTempFile("pet-segment-", ".json", getApplication<Application>().cacheDir)
                try {
                    staged.outputStream().buffered().use { pet.exportRecording(it, completed = true) }
                    files.save(next, staged, kotlin.math.round(pet.timeMs * 30 / 1000).toLong())
                    pet.commitSegment()
                } finally { staged.delete() }
            } else files.save(pet.recording())
            mutable.update { it.copy(savedAtMs = pet.timeMs, archives = if (next != null) files.archives() else it.archives) }
            return true
        } catch (e: Exception) {
            saveBlocked = true
            mutable.update { it.copy(message = "Saving paused; the previous file is kept. ${e.message}") }
            return false
        }
    }
    private fun open(nextMode: PetMode) {
        val nextStore = PetHabitatStore(getApplication<Application>().filesDir, nextMode.key)
        var blocked = false
        val saved = try { nextStore.read() } catch (e: Exception) { blocked = true; message(e); null }
        val next = try { PetNativeCore(bundle, points, tape(nextMode), saved, live = nextMode == PetMode.LIVE) } catch (e: Exception) {
            blocked = true
            mutable.update { it.copy(message = "Saved habitat could not be opened and is kept intact. ${e.message}") }
            PetNativeCore(bundle, points, tape(nextMode), live = nextMode == PetMode.LIVE)
        }
        stopAudio(); stopLive(); core?.close(); core = next; store = nextStore; saveBlocked = blocked; mode = nextMode
        prefs.edit().putString("mode", nextMode.key).apply()
        mutable.update { it.copy(scene = next.scene(), mode = nextMode, savedAtMs = if (saved != null && !blocked) next.timeMs else null,
            canExportRecovery = recoveryFile() != null, archives = nextStore.archives()) }
        lastSave = SystemClock.elapsedRealtime()
    }
    private fun handle(command: Command) {
        when (command) {
            is Command.Mode -> if (command.mode != mode) {
                check(command.mode != PetMode.LIVE || liveUri != null) { "Choose Follow local tape from More first." }
                check(save()) { "This world could not be saved. Export it or reopen the saved habitat before switching." }
                open(command.mode)
            }
            is Command.Follow -> {
                require(command.uri.scheme == "content") { "Choose a local document through the file picker." }
                check(save()) { "This world could not be saved. Export it before changing sources." }
                open(PetMode.LIVE)
                liveUri = command.uri
                runCatching { getApplication<Application>().contentResolver.takePersistableUriPermission(command.uri, Intent.FLAG_GRANT_READ_URI_PERMISSION) }
                prefs.edit().putString("live-uri", command.uri.toString()).apply()
                mutable.update { it.copy(canFollowLive = true, message = "Waiting for fresh local telemetry. Existing file contents are not replayed as live work.") }
            }
            is Command.Interact -> core?.interact(command.food)
            is Command.Reload -> { open(mode); setPaused(false) }
            is Command.Restart -> {
                save()
                val replay = if (mode == PetMode.RECORDING) core?.recording()?.let { JSONObject(it).apply { remove("checkpoint") }.toString() } else null
                val next = PetNativeCore(bundle, points, tape(mode), replay, live = mode == PetMode.LIVE)
                try {
                    val backup = checkNotNull(store).restart(next.recording())
                    if (backup != null) prefs.edit().putString("recovery-${mode.key}", backup.name).apply()
                    stopAudio(); stopLive(); core?.close(); core = next; saveBlocked = false
                    mutable.update { it.copy(scene = next.scene(), savedAtMs = next.timeMs, canExportRecovery = recoveryFile() != null,
                        message = "Fresh habitat started. You can export the previous saved world from More.") }
                    lastSave = SystemClock.elapsedRealtime()
                } catch (e: Exception) { next.close(); throw e }
            }
            is Command.Import -> {
                val resolver = getApplication<Application>().contentResolver
                val saved = resolver.openInputStream(command.uri)?.use(PetHabitatStore::boundedRead) ?: error("Could not open the selected recording.")
                val next = PetNativeCore(bundle, points, saved = saved)
                try {
                    check(save()) { "This world could not be saved. Export it before importing another recording." }
                    val scene = next.scene()
                    val nextStore = PetHabitatStore(getApplication<Application>().filesDir, PetMode.RECORDING.key)
                    nextStore.read(); nextStore.save(next.recording())
                    stopAudio(); stopLive(); core?.close(); core = next; store = nextStore; saveBlocked = false; mode = PetMode.RECORDING
                    prefs.edit().putString("mode", mode.key).apply()
                    mutable.update { it.copy(scene = scene, mode = mode, savedAtMs = next.timeMs) }
                    lastSave = SystemClock.elapsedRealtime()
                } catch (e: Exception) { next.close(); throw e }
            }
            is Command.Export -> {
                val app = getApplication<Application>()
                val staged = File.createTempFile("pet-export-", ".json", app.cacheDir)
                try {
                    val bytes = staged.outputStream().buffered().use { checkNotNull(core).exportRecording(it) }
                    // Finish and validate the snapshot before opening the user's
                    // destination. A core/size failure cannot truncate that file.
                    val out = app.contentResolver.openOutputStream(command.uri, "wt") ?: error("Could not open the selected export file.")
                    out.use { output -> staged.inputStream().use { it.copyTo(output) } }
                    mutable.update { it.copy(message = if (bytes > PetNativeCore.MAX_HABITAT_BYTES)
                        "Recording exported. Open files over 8 MiB in the browser."
                        else "Recording exported, including the current world state.") }
                } finally { staged.delete() }
            }
            is Command.ExportArchive -> {
                val file = checkNotNull(store).archive(command.name)
                val out = getApplication<Application>().contentResolver.openOutputStream(command.uri, "wt")
                    ?: error("Could not open the selected export file.")
                out.use { output -> file.inputStream().use { it.copyTo(output) } }
                mutable.update { it.copy(message = "Earlier recording exported.") }
            }
            is Command.ExportRecovery -> {
                val file = recoveryFile() ?: error("No previous world is available.")
                val out = getApplication<Application>().contentResolver.openOutputStream(command.uri, "wt")
                    ?: error("Could not open the selected export file.")
                out.use { output -> file.inputStream().use { it.copyTo(output) } }
                mutable.update { it.copy(message = "Previous saved world exported.") }
            }
        }
    }
    private fun recoveryFile(): File? {
        val name = prefs.getString("recovery-${mode.key}", null) ?: return null
        if (!name.matches(Regex("pet-${mode.key}-recovery-[0-9a-f-]+\\.json"))) return null
        return File(getApplication<Application>().filesDir, name).takeIf { it.isFile }
    }
}
