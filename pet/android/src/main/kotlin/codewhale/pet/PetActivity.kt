package codewhale.pet

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.database.ContentObserver
import android.media.AudioManager
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.collectAsStateWithLifecycle

class PetActivity : ComponentActivity() {
    private val model by viewModels<PetViewModel>()
    private val noise = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) { model.setSound(false) }
    }
    private val motion = object : ContentObserver(Handler(Looper.getMainLooper())) {
        override fun onChange(selfChange: Boolean) = readMotion()
    }
    private fun readMotion() {
        model.setSystemStill(Settings.Global.getFloat(contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f)
    }
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge(statusBarStyle = SystemBarStyle.dark(android.graphics.Color.TRANSPARENT),
            navigationBarStyle = SystemBarStyle.dark(android.graphics.Color.TRANSPARENT))
        readMotion()
        contentResolver.registerContentObserver(Settings.Global.getUriFor(Settings.Global.ANIMATOR_DURATION_SCALE), false, motion)
        ContextCompat.registerReceiver(this, noise, IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY), ContextCompat.RECEIVER_NOT_EXPORTED)
        setContent {
            MaterialTheme(colorScheme = darkColorScheme(primary = Color(0xff73c9b5), background = Color(0xff071319),
                surface = Color(0xff071319), onSurface = Color(0xffd9e8e6), onSurfaceVariant = Color(0xff9aacaf))) {
                Surface(Modifier.fillMaxSize()) { PetScreen(model) }
            }
        }
    }
    override fun onResume() { super.onResume(); readMotion(); model.setActive(true) }
    override fun onPause() { model.setActive(false); super.onPause() }
    override fun onDestroy() {
        contentResolver.unregisterContentObserver(motion); unregisterReceiver(noise)
        super.onDestroy()
    }
}

@Composable
private fun PetScreen(model: PetViewModel) {
    val ui by model.ui.collectAsStateWithLifecycle()
    val import = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { it?.let(model::importRecording) }
    val export = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/json")) { it?.let(model::exportRecording) }
    val recovery = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/json")) { it?.let(model::exportRecovery) }
    var restart by remember { mutableStateOf(false) }
    if (restart) AlertDialog(onDismissRequest = { restart = false }, title = { Text("Start a fresh habitat?") },
        text = { Text("The current saved world will be kept as a recovery copy. You can export it from More.") },
        confirmButton = { TextButton(onClick = { restart = false; model.restart() }) { Text("Start fresh") } },
        dismissButton = { TextButton(onClick = { restart = false }) { Text("Cancel") } })
    Column(Modifier.safeDrawingPadding().fillMaxSize()) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 22.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            Text("Codewhale", style = MaterialTheme.typography.headlineSmall, modifier = Modifier.weight(1f))
            var menu by remember { mutableStateOf(false) }
            Box {
                TextButton(onClick = { menu = true }) { Text("More") }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    DropdownMenuItem(text = { Text("Import recording") }, onClick = { menu = false; import.launch(arrayOf("application/json", "text/plain", "application/octet-stream")) })
                    DropdownMenuItem(text = { Text("Export recording") }, enabled = ui.scene != null, onClick = { menu = false; export.launch("codewhale-pet.json") })
                    DropdownMenuItem(text = { Text("Reopen saved habitat") }, onClick = { menu = false; model.reload() })
                    DropdownMenuItem(text = { Text("Start fresh habitat") }, enabled = ui.scene != null, onClick = { menu = false; restart = true })
                    DropdownMenuItem(text = { Text("Export previous world") }, enabled = ui.canExportRecovery,
                        onClick = { menu = false; recovery.launch("codewhale-pet-recovery.json") })
                }
            }
        }
        BoxWithConstraints(Modifier.weight(1f).fillMaxWidth()) {
            val controlsHeight = maxHeight * 0.55f
            if (maxWidth > maxHeight && maxWidth > 540.dp) {
                Row(Modifier.fillMaxSize()) {
                    Habitat(ui, Modifier.weight(1f).fillMaxHeight())
                    Controls(ui, model, Modifier.width(256.dp).fillMaxHeight().verticalScroll(rememberScrollState()))
                }
            } else {
                Column(Modifier.fillMaxSize()) {
                    Habitat(ui, Modifier.weight(1f).fillMaxWidth())
                    Controls(ui, model, Modifier.fillMaxWidth().heightIn(max = controlsHeight).verticalScroll(rememberScrollState()))
                }
            }
        }
    }
}

@Composable
private fun Habitat(ui: PetUiState, modifier: Modifier) {
    Column(modifier) {
        Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
            ui.scene?.let { CodewhalePet(it) } ?: if (ui.message == null) CircularProgressIndicator()
                else Text("The habitat could not open.", color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        ui.scene?.let { scene ->
            Text("${scene.style.channel} · ${scene.style.arch}${if (scene.style.hollow) " · unobserved" else ""}",
                Modifier.padding(horizontal = 22.dp, vertical = 10.dp), fontFamily = FontFamily.Monospace,
                style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Controls(ui: PetUiState, model: PetViewModel, modifier: Modifier) {
    Column(modifier.padding(horizontal = 18.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            listOf(PetMode.WILD, PetMode.DEMO).forEach { mode ->
                FilterChip(selected = ui.mode == mode, onClick = { model.mode(mode) }, label = { Text(mode.label) })
            }
            if (ui.mode == PetMode.RECORDING) FilterChip(selected = true, onClick = {}, label = { Text("Recording") })
        }
        Text(ui.mode.detail, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(enabled = ui.scene != null, onClick = { model.setPaused(!ui.paused) }) { Text(if (ui.paused) "Resume" else "Pause") }
            OutlinedButton(enabled = ui.scene != null, onClick = { model.setSound(!ui.sound) }) { Text(if (ui.sound) "Sound on" else "Sound off") }
            OutlinedButton(enabled = !ui.systemStill, onClick = { model.setStill(!ui.still) }) {
                Text(if (ui.systemStill) "System still" else if (ui.still) "Still on" else "Still off")
            }
        }
        Row {
            TextButton(onClick = { model.interact(true) }, enabled = ui.scene != null) { Text("Offer food") }
            TextButton(onClick = { model.interact(false) }, enabled = ui.scene != null) { Text("Interact") }
        }
        ui.scene?.let {
            val seconds = (it.timeMs / 1000).toLong()
            Text("${seconds / 60}:${(seconds % 60).toString().padStart(2, '0')} · ${it.behaviour}${if (it.peers >= 3) " · ${it.peers} pod members" else ""}",
                style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        ui.message?.let { message ->
            Surface(color = MaterialTheme.colorScheme.surfaceContainer, shape = MaterialTheme.shapes.small) {
                Column(Modifier.padding(12.dp)) {
                    Text(message, style = MaterialTheme.typography.bodySmall)
                    TextButton(onClick = model::dismissMessage) { Text("Dismiss") }
                }
            }
        }
    }
}
