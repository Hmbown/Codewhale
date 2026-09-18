package codewhale.pet

import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Test
import org.junit.Assume.assumeTrue
import org.junit.Assert.*
import org.junit.runner.RunWith
import org.json.JSONObject
import java.io.File

/** Opt-in real companion check. The fixture descriptor is injected into this
 * debug app's private files; a secure adb reverse carries loopback traffic. */
@RunWith(AndroidJUnit4::class)
class PetSharedIntegrationTest {
    @Test fun twoMobileViewsAttachWithoutAWorldAndInteractionAppearsInOwnerRecording() {
        val app=InstrumentationRegistry.getInstrumentation().targetContext
        val descriptor=File(app.filesDir,"shared-test-connection.json")
        assumeTrue("No explicit local companion fixture",descriptor.isFile)
        val first=PetSharedClient(descriptor);val second=PetSharedClient(descriptor)
        val a=first.poll(false,false);val b=second.poll(true,false)
        assertEquals(first.identity.substringBefore(" ·"),second.identity.substringBefore(" ·"))
        assertEquals(1960,a.dots.size);assertEquals(1960,b.dots.size)
        val before=JSONObject(String(first.export())).getJSONArray("interactions").length()
        second.poll(false,false) // Refresh after export before an interaction's freshness check.
        second.interact(true)
        val deadline=android.os.SystemClock.elapsedRealtime()+5_000
        var after=before
        while(after==before && android.os.SystemClock.elapsedRealtime()<deadline) {
            Thread.sleep(50)
            after=JSONObject(String(first.export())).getJSONArray("interactions").length()
        }
        assertEquals(before+1,after)
        first.detach();Thread.sleep(150)
        assertTrue(second.poll(false,false).timeMs>b.timeMs)
        second.detach()
    }
}
