package codewhale.pet

import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.ViewModelProvider
import android.net.Uri
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test

class PetLifecycleTest {
    @get:Rule val compose = createAndroidComposeRule<PetActivity>()

    @Test fun pauseStillSoundAndBackgroundLifecycleUseTheVisibleWorld() {
        val model = ViewModelProvider(compose.activity)[PetViewModel::class.java]
        compose.waitUntil(15_000) { model.ui.value.scene != null }
        assertFalse(model.ui.value.sound)
        compose.runOnIdle { model.setStill(false); model.setPaused(false) }
        compose.onNodeWithText("Pause").performClick()
        compose.waitUntil(5_000) { model.ui.value.paused && !model.ui.value.running }
        val paused = model.ui.value.scene!!
        Thread.sleep(300)
        assertEquals(paused.timeMs, model.ui.value.scene!!.timeMs, 0.0)
        assertArrayEquals(paused.dots, model.ui.value.scene!!.dots, 0f)
        compose.onNodeWithText("More").performClick()
        compose.onNodeWithText("Reopen saved habitat").performClick()
        compose.onNodeWithText("Reopen the saved habitat?").assertExists()
        compose.onNodeWithText("Cancel").performClick()
        assertEquals(paused.timeMs, model.ui.value.scene!!.timeMs, 0.0)
        compose.onNodeWithText("Still off").performClick()
        compose.onNodeWithText("Sound off").performClick()
        compose.onNodeWithText("Resume").performClick()
        compose.waitUntil(5_000) { model.ui.value.still && model.ui.value.scene!!.timeMs > paused.timeMs + 200 }
        assertTrue(model.ui.value.sound)
        Thread.sleep(100)
        val still = model.ui.value.scene!!
        Thread.sleep(200)
        assertArrayEquals(still.dots, model.ui.value.scene!!.dots, 0f)
        compose.activityRule.scenario.moveToState(Lifecycle.State.CREATED)
        compose.waitUntil(5_000) { !model.ui.value.running && model.ui.value.savedAtMs == model.ui.value.scene!!.timeMs }
        val hidden = model.ui.value.scene!!
        Thread.sleep(300)
        assertEquals(hidden.timeMs, model.ui.value.scene!!.timeMs, 0.0)
        assertFalse(Thread.getAllStackTraces().keys.any { it.name == "codewhale-pet-audio" && it.isAlive })
        assertEquals(hidden.timeMs, model.ui.value.savedAtMs!!, 0.0)
        compose.activityRule.scenario.moveToState(Lifecycle.State.RESUMED)
        compose.waitUntil(5_000) { model.ui.value.scene!!.timeMs > hidden.timeMs }
        compose.onNodeWithText("Sound on").performClick()
        compose.onNodeWithText("Still on").performClick()
        assertFalse(model.ui.value.sound)
    }

    @Test fun localLiveFileStaysFreshAcrossPauseBackgroundAndProducerRestart() {
        val model = ViewModelProvider(compose.activity)[PetViewModel::class.java]
        compose.waitUntil(15_000) { model.ui.value.scene != null }
        val resolver = compose.activity.contentResolver
        val uri = Uri.parse("content://dev.shannonlabs.codewhale.pet.test.live/tape")
        val human = compose.activity.assets.open("demo.jsonl").bufferedReader().use { input ->
            input.lineSequence().map(::JSONObject).first { it.getString("channel") == "human" && it.getBoolean("waiting") }.toString()
        }
        fun packet(sequence: Int) = JSONObject(human).put("sequence", sequence).put("simTimeMs", sequence * 400).toString() + "\n"
        fun action(name: String, text: String? = null) = resolver.call(uri, name, text, null)!!.getInt("reads")
        fun observed() = model.ui.value.scene!!.state.observed >= .92
        action("replace", packet(10))
        val initialReads = action("stats")
        compose.runOnIdle { model.setPaused(false); model.followLocalTape(uri) }
        try {
            compose.waitUntil(10_000) { model.ui.value.mode == PetMode.LIVE && action("stats") >= initialReads + 2 }
            assertFalse("Existing file must establish a baseline", observed())
            action("append", packet(11))
            compose.waitUntil(5_000) { observed() && model.ui.value.scene!!.state.channel == "human" }
            compose.onNodeWithText("Pause").performClick()
            compose.waitUntil(5_000) { model.ui.value.paused && !model.ui.value.running }
            val stoppedReads = action("stats")
            action("append", packet(12)); Thread.sleep(600)
            assertEquals("Pause closes live polling", stoppedReads, action("stats"))
            compose.onNodeWithText("Resume").performClick()
            compose.waitUntil(5_000) { model.ui.value.running && action("stats") >= stoppedReads + 2 }
            assertFalse("Background bytes are not fresh requests", observed())
            assertEquals("none", model.ui.value.scene!!.needs)
            action("append", packet(13))
            compose.waitUntil(5_000) { observed() }
            compose.waitUntil(5_000) { !observed() }
            val beforeRestart = action("stats")
            action("replace", packet(0))
            compose.waitUntil(5_000) { action("stats") >= beforeRestart + 2 }
            assertFalse(observed())
            action("append", packet(1))
            compose.waitUntil(5_000) { observed() }
            compose.activityRule.scenario.moveToState(Lifecycle.State.CREATED)
            compose.waitUntil(5_000) { !model.ui.value.running }
            val backgroundReads = action("stats")
            action("append", packet(2)); Thread.sleep(600)
            assertEquals(backgroundReads, action("stats"))
            compose.activityRule.scenario.moveToState(Lifecycle.State.RESUMED)
            compose.waitUntil(5_000) { model.ui.value.running && action("stats") >= backgroundReads + 2 }
            assertFalse(observed())
            val beforeError = model.ui.value.scene!!.timeMs
            action("replace", "{invalid}\n")
            compose.waitUntil(5_000) { model.ui.value.message?.startsWith("Invalid local telemetry") == true }
            compose.waitUntil(5_000) { model.ui.value.scene!!.timeMs > beforeError + 800 }
            assertFalse(model.ui.value.paused)
            assertFalse(observed())
        } finally {
            compose.activityRule.scenario.moveToState(Lifecycle.State.RESUMED)
            compose.runOnIdle { model.mode(PetMode.WILD) }
            compose.waitUntil(5_000) { model.ui.value.mode == PetMode.WILD }
            // Settle this activity's final frame/save before the next test opens
            // the same private habitat with a new ViewModel and store revision.
            compose.runOnIdle { model.setPaused(true) }
            compose.waitUntil(5_000) { model.ui.value.paused && !model.ui.value.running && model.ui.value.savedAtMs == model.ui.value.scene!!.timeMs }
            action("delete")
        }
    }
}
