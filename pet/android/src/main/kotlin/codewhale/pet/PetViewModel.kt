package codewhale.pet

import android.app.Application
import android.net.Uri
import android.os.SystemClock
import java.io.File
import org.json.JSONObject
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import java.util.concurrent.Executors
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.NonCancellable
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
}
data class PetUiState(val scene: PetScene? = null, val mode: PetMode = PetMode.WILD,
    val paused: Boolean = false, val sound: Boolean = false, val still: Boolean = false,
    val systemStill: Boolean = false, val message: String? = null, val savedAtMs: Double? = null,
    val canExportRecovery: Boolean = false)

/** A single actor owns core, audio cursor and save revision. Compose receives
 * immutable projections and never reads a particle while it is being stepped. */
class PetViewModel(application: Application) : AndroidViewModel(application) {
    private sealed interface Command {
        data class Mode(val mode: PetMode) : Command
        data class Interact(val food: Boolean) : Command
        data class Import(val uri: Uri) : Command
        data class Export(val uri: Uri) : Command
        data class ExportRecovery(val uri: Uri) : Command
        data object Restart : Command
        data object Reload : Command
    }
    private val prefs = application.getSharedPreferences("pet", 0)
    private val initialMode = PetMode.entries.firstOrNull { it.key == prefs.getString("mode", "wild") } ?: PetMode.WILD
    private val mutable = MutableStateFlow(PetUiState(mode = initialMode, still = prefs.getBoolean("still", false)))
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
                            val playing = active && !paused
                            if (!playing) {
                                stopAudio()
                                if (wasPlaying) save()
                            } else {
                                val pet = checkNotNull(core)
                                if (!sound) stopAudio()
                                if (sound && output == null) {
                                    try {
                                        output = PetAudioOutput(application) { setSound(false) }
                                        cursor = PetAudioCursor(pet.timeMs)
                                    } catch (e: Exception) { audioFailed(e) }
                                }
                                val scene = pet.tick(!still && !systemStill)
                                if (output != null) {
                                    try { cursor?.next(pet)?.let { output?.offer(it) } }
                                    catch (e: Exception) { audioFailed(e) }
                                }
                                mutable.update { it.copy(scene = scene) }
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
                    stopAudio(); runCatching { save() }; core?.close(); core = null
                }
                commands.close(); worker.close()
            }
        }
    }

    fun setActive(value: Boolean) { active = value }
    fun setPaused(value: Boolean) { paused = value; mutable.update { it.copy(paused = value) } }
    fun setSound(value: Boolean) { sound = value; mutable.update { it.copy(sound = value) } }
    fun setStill(value: Boolean) { still = value; prefs.edit().putBoolean("still", value).apply(); mutable.update { it.copy(still = value) } }
    fun setSystemStill(value: Boolean) { systemStill = value; mutable.update { it.copy(systemStill = value) } }
    fun mode(value: PetMode) = enqueue(Command.Mode(value))
    fun interact(food: Boolean) = enqueue(Command.Interact(food))
    fun importRecording(uri: Uri) = enqueue(Command.Import(uri))
    fun exportRecording(uri: Uri) = enqueue(Command.Export(uri))
    fun exportRecovery(uri: Uri) = enqueue(Command.ExportRecovery(uri))
    fun restart() = enqueue(Command.Restart)
    fun reload() = enqueue(Command.Reload)
    fun dismissMessage() { mutable.update { it.copy(message = null) } }
    private fun enqueue(command: Command) {
        if (!commands.trySend(command).isSuccess) mutable.update { it.copy(message = "The habitat is busy. Try again.") }
    }
    private fun message(e: Exception) { mutable.update { it.copy(message = e.message?.take(400) ?: "The habitat could not continue.") } }
    private fun stopAudio() { output?.close(); output = null; cursor = null }
    private fun audioFailed(e: Exception) { stopAudio(); setSound(false); mutable.update { it.copy(message = "Sound paused: ${e.message ?: "output unavailable"}") } }
    private fun save(): Boolean {
        lastSave = SystemClock.elapsedRealtime()
        if (saveBlocked) return false
        val pet = core ?: return true
        try {
            store?.save(pet.recording())
            mutable.update { it.copy(savedAtMs = pet.timeMs) }
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
        val next = try { PetNativeCore(bundle, points, tape(nextMode), saved) } catch (e: Exception) {
            blocked = true
            mutable.update { it.copy(message = "Saved habitat could not be opened and is kept intact. ${e.message}") }
            PetNativeCore(bundle, points, tape(nextMode))
        }
        stopAudio(); core?.close(); core = next; store = nextStore; saveBlocked = blocked; mode = nextMode
        prefs.edit().putString("mode", nextMode.key).apply()
        mutable.update { it.copy(scene = next.scene(), mode = nextMode, savedAtMs = if (saved != null && !blocked) next.timeMs else null,
            canExportRecovery = recoveryFile() != null) }
        lastSave = SystemClock.elapsedRealtime()
    }
    private fun handle(command: Command) {
        when (command) {
            is Command.Mode -> if (command.mode != mode) {
                check(save()) { "This world could not be saved. Export it or reopen the saved habitat before switching." }
                open(command.mode)
            }
            is Command.Interact -> core?.interact(command.food)
            is Command.Reload -> { open(mode); setPaused(false) }
            is Command.Restart -> {
                save()
                val replay = if (mode == PetMode.RECORDING) core?.recording()?.let { JSONObject(it).apply { remove("checkpoint") }.toString() } else null
                val next = PetNativeCore(bundle, points, tape(mode), replay)
                try {
                    val backup = checkNotNull(store).restart(next.recording())
                    if (backup != null) prefs.edit().putString("recovery-${mode.key}", backup.name).apply()
                    stopAudio(); core?.close(); core = next; saveBlocked = false
                    mutable.update { it.copy(scene = next.scene(), savedAtMs = 0.0, canExportRecovery = recoveryFile() != null,
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
                    stopAudio(); core?.close(); core = next; store = nextStore; saveBlocked = false; mode = PetMode.RECORDING
                    prefs.edit().putString("mode", mode.key).apply()
                    mutable.update { it.copy(scene = scene, mode = mode, savedAtMs = next.timeMs) }
                    lastSave = SystemClock.elapsedRealtime()
                } catch (e: Exception) { next.close(); throw e }
            }
            is Command.Export -> {
                val data = checkNotNull(core).recording()
                val resolver = getApplication<Application>().contentResolver
                val out = resolver.openOutputStream(command.uri, "wt") ?: error("Could not open the selected export file.")
                out.bufferedWriter(Charsets.UTF_8).use { it.write(data) }
                mutable.update { it.copy(message = "Recording exported.") }
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
