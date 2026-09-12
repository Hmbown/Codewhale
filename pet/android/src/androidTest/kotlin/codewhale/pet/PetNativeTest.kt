package codewhale.pet

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.File
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class PetNativeTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private fun asset(name: String) = context.assets.open(name).bufferedReader().use { it.readText() }
    private fun core(saved: String? = null) = PetNativeCore(asset("pet-native.js"), asset("whale-points.tsv"), asset("demo.jsonl"), saved)

    @Test fun nativeRendererMatchesEverySharedWorldFrameAndResumesCheckpoint() {
        for (motion in listOf(true, false)) {
            core().use { world ->
                var resumed: PetNativeCore? = null
                try {
                    var voices = 0
                    for (tick in 0 until 2400) {
                        if (tick == 400) world.interact(true)
                        val frame = world.tick(motion)
                        voices += world.voices.length()
                        assertEquals("Kotlin vs JS: $motion / $tick", frame.digest, petDigest(world.sim))
                        resumed?.let {
                            val replay = it.tick(motion)
                            assertEquals("Checkpoint continuation: $motion / $tick", frame.digest, replay.digest)
                            assertEquals(frame.behaviour, replay.behaviour)
                            assertEquals(world.voices.toString(), it.voices.toString())
                            assertEquals(replay.digest, petDigest(it.sim))
                        }
                        if (tick == 899) resumed = core(world.recording())
                    }
                    assertTrue("Fixture includes the actual score", voices > 0)
                } finally { resumed?.close() }
            }
        }
    }

    @Test fun invalidAndLegacyRecordingsRespectTheVersionBoundary() {
        core().use { original ->
            val before = original.recording()
            val bad = JSONObject(before).put("expressionVersion", 3).toString()
            assertThrows(Exception::class.java) { core(bad).close() }
            val badBody = JSONObject(before)
            badBody.getJSONObject("checkpoint").getJSONObject("sim").getJSONArray("body").getJSONArray(0).put(0, 0.99)
            assertThrows(Exception::class.java) { core(badBody.toString()).close() }
            assertEquals(before, original.recording())
            val exported = JSONObject(before).apply { remove("checkpoint") }.toString()
            core(exported).use { replay ->
                assertEquals(2, replay.sim.expressionVersion)
                assertEquals(0.0, replay.timeMs, 0.0)
                repeat(120) { assertEquals(replay.tick(true).digest, petDigest(replay.sim)) }
            }
            // At t=0 both expression versions have the identical home body.
            val legacy = JSONObject(before).apply {
                put("petReplayVersion", 1); remove("start")
                remove("expressionVersion")
                getJSONObject("checkpoint").apply {
                    put("petCheckpointVersion", 1); remove("historyStart"); remove("hasTelemetry")
                    getJSONObject("sim").remove("expressionVersion")
                }
            }
            core(legacy.toString()).use { restored ->
                assertEquals(1, restored.sim.expressionVersion)
                repeat(120) { assertEquals(restored.tick(true).digest, petDigest(restored.sim)) }
            }
        }
    }

    @Test fun nativeAudioCursorMatchesWholeScoreWithoutReplayingPastVoices() {
        core().use { world ->
            val cursor = PetAudioCursor(world.timeMs)
            val all = JSONArray()
            val chunks = mutableListOf<FloatArray>()
            repeat(30) {
                world.tick(true)
                for (i in 0 until world.voices.length()) all.put(world.voices.getJSONObject(i))
                chunks += checkNotNull(cursor.next(world))
            }
            val received = FloatArray(chunks.sumOf { it.size })
            var offset = 0
            chunks.forEach { it.copyInto(received, offset); offset += it.size }
            val whole = world.pcm(all, 0, 24_000) + world.pcm(all, 24_000, 24_000)
            assertArrayEquals(whole, received, 0f)
            assertTrue(whole.any { kotlin.math.abs(it) > 0.001f })
            assertNull(PetAudioCursor(world.timeMs).next(world))
            assertThrows(IllegalArgumentException::class.java) { world.pcm(all, -1, 100) }
            assertThrows(IllegalArgumentException::class.java) { world.pcm(all, 0, 24_001) }
        }
    }

    @Test fun longParticleClocksRestoreAndContinueInBothEngines() {
        // Construct a long-clock boundary fixture; this is not a 30-hour device run.
        core().use { original ->
            val saved = JSONObject(original.recording())
            val particle = saved.getJSONObject("checkpoint").getJSONObject("sim")
            particle.put("clock", 108_000.0).put("phase", 108_000.0)
            particle.getJSONArray("particles").getJSONArray(0).put(4, 1_000_001.0)
            core(saved.toString()).use { restored ->
                repeat(60) { assertEquals(restored.tick(true).digest, petDigest(restored.sim)) }
                core(restored.recording()).use { again ->
                    repeat(60) { assertEquals(restored.tick(true).digest, again.tick(true).digest) }
                }
            }
            particle.put("clock", 100.0 * 365.0 * 86_400.0 + 1.0)
            assertThrows(Exception::class.java) { core(saved.toString()).close() }
        }
    }

    @Test fun exportRetainsTheExactUnsavedVisitAfterAStoreConflict() {
        val directory = File(context.cacheDir, "pet-export-${java.util.UUID.randomUUID()}").apply { mkdirs() }
        try {
            core().use { world ->
                val store = PetHabitatStore(directory, "wild")
                assertNull(store.read()); store.save(world.recording())
                repeat(31) { world.tick(true) }
                world.interact(true)
                File(directory, "pet-wild.json").writeText("other writer")
                assertThrows(IllegalStateException::class.java) { store.save(world.recording()) }
                val output = ByteArrayOutputStream()
                assertEquals(world.exportRecording(output), output.size().toLong())
                val saved = output.toString("UTF-8")
                assertEquals(world.recording(), saved)
                core(saved).use { restored ->
                    assertEquals(world.timeMs, restored.timeMs, 0.0)
                    repeat(60) { assertEquals(world.tick(true).digest, restored.tick(true).digest) }
                }
                assertEquals("other writer", File(directory, "pet-wild.json").readText())
            }
        } finally { directory.deleteRecursively() }
    }

    @Test fun aRecordingBeyondTheAutosaveLimitCanStillBeExportedInChunks() {
        // Populate through the actual public interaction API, inside the same
        // 64 MiB QuickJS heap. The subclass only seeds this large input fixture.
        val fixture = asset("pet-native.js") + """
            globalThis.PetNative = class extends globalThis.PetNative {
                constructor(...args) {
                    super(...args);
                    for (let i = 0; i < 90000; i++) this.interact('attention', 0.3141592653589793, -0.2718281828459045);
                }
            };
        """.trimIndent()
        PetNativeCore(fixture, asset("whale-points.tsv")).use { world ->
            assertThrows(IllegalArgumentException::class.java) { world.recording() }
            val file = File.createTempFile("pet-large-export-", ".json", context.cacheDir)
            try {
                val size = file.outputStream().buffered().use { world.exportRecording(it) }
                assertEquals(size, file.length())
                assertTrue(size > PetNativeCore.MAX_HABITAT_BYTES)
                val recording = JSONObject(file.readText())
                assertEquals(90000, recording.getJSONArray("interactions").length())
                assertEquals(0, recording.getJSONObject("checkpoint").getInt("tick"))
                assertEquals(2, recording.getInt("expressionVersion"))
                // Export has not advanced or discarded the in-memory visit.
                assertEquals(0.0, world.timeMs, 0.0)
            } finally { file.delete() }
        }
    }

    @Test fun atomicStorageRefusesConflictingWritersAndOversizedOrInvalidUtf8Input() {
        val directory = File(context.cacheDir, "pet-store-${java.util.UUID.randomUUID()}").apply { mkdirs() }
        try {
            val a = PetHabitatStore(directory, "wild")
            val b = PetHabitatStore(directory, "wild")
            assertNull(a.read()); assertNull(b.read())
            a.save("first")
            assertThrows(IllegalStateException::class.java) { b.save("stale") }
            assertEquals("first", b.read())
            b.save("second")
            assertThrows(IllegalStateException::class.java) { a.save("stale") }
            assertEquals("second", a.read())
            // A malformed UTF-8 original can still be preserved and restarted.
            val damaged = File(directory, "pet-demo.json")
            val bytes = byteArrayOf(0xc3.toByte(), 0x28)
            damaged.writeBytes(bytes)
            val recovery = PetHabitatStore(directory, "demo")
            assertThrows(java.nio.charset.CharacterCodingException::class.java) { recovery.read() }
            val backup = recovery.restart("fresh")!!
            assertArrayEquals(bytes, backup.readBytes())
            assertEquals("fresh", recovery.read())
            assertThrows(IllegalArgumentException::class.java) {
                PetHabitatStore.boundedRead(ByteArrayInputStream(ByteArray(PetNativeCore.MAX_HABITAT_BYTES + 1)))
            }
            assertThrows(java.nio.charset.CharacterCodingException::class.java) {
                PetHabitatStore.boundedRead(ByteArrayInputStream(byteArrayOf(0xc3.toByte(), 0x28)))
            }
            assertEquals("second", a.read())
        } finally { directory.deleteRecursively() }
    }

    @Test fun rotationArchivesBeforeRetiringHistoryAndKeepsTheWorldOnAStorageConflict() {
        val directory = File(context.cacheDir, "pet-segment-${java.util.UUID.randomUUID()}").apply { mkdirs() }
        val seeded = asset("pet-native.js") + "\nconst SegmentBase = PetNative; PetNative = class extends SegmentBase { constructor(...args) { super(...args); for(let i=0;i<4096;i++) this.interact('food', .2, -.15); } };"
        try {
            PetNativeCore(seeded, asset("whale-points.tsv")).use { world ->
                world.tick(false)
                val original = world.recording(); val before = world.scene()
                val next = world.prepareSegment()!!
                val staged = File(directory, "staged.json")
                staged.outputStream().use { world.exportRecording(it, completed = true) }
                val store = PetHabitatStore(directory, "wild")
                assertNull(store.read()); store.save(original)
                val habitat = File(directory, "pet-wild.json")
                habitat.writeText("another writer")
                assertThrows(IllegalStateException::class.java) { store.save(next, staged, 1) }
                assertEquals(original, world.recording()); assertTrue(store.archives().isEmpty())
                assertEquals("another writer", habitat.readText())
                habitat.writeText(original)
                store.save(next, staged, 1); world.commitSegment()
                assertEquals(before.digest, world.scene().digest)
                assertTrue(world.recording().length < original.length)
                assertEquals(1, store.archives().size)
                assertEquals(original, store.archive(store.archives().single()).readText())
                core(store.read()).use { restored ->
                    assertEquals(before.digest, restored.scene().digest)
                    repeat(60) { assertEquals(world.tick(true).digest, restored.tick(true).digest) }
                }
                val archive = store.archive(store.archives().single()); archive.writeText("damaged")
                assertThrows(IllegalStateException::class.java) { store.archive(store.archives().single()) }
                assertEquals("damaged", archive.readText())
            }
        } finally { directory.deleteRecursively() }
    }
}
