package codewhale.pet

import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.ViewModelProvider
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
}
