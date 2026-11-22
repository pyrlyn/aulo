package aulo.telemetry

import fs.TempDir
import fs.entryKind
import fs.listDir
import fs.readText
import kotlinx.io.files.Path
import kotlin.test.AfterTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull

class RotatingFileSinkTest {
    private val tmp = TempDir.create(".aulo-telemetry-test")
    private val dir = logDir(tmp.path)

    @AfterTest
    fun cleanup() = tmp.close()

    @Test
    fun linesAppendUnderTheLogDirectory() {
        RotatingFileSink(dir).use { sink ->
            sink.write("one")
            sink.write("two")
        }
        RotatingFileSink(dir).use { it.write("three") }
        assertEquals("one\ntwo\nthree\n", readText(Path(dir, "aulo.log")))
    }

    @Test
    fun rotationCapsSizeAndKeepsOnlyTheNewestFiles() {
        // Each line is 10 bytes with its newline, so two fit under the 25-byte cap.
        RotatingFileSink(dir, maxBytes = 25, keep = 2).use { sink ->
            for (i in 0 until 7) sink.write("line-${i}xxx")
        }
        assertEquals(listOf("aulo.log", "aulo.log.1", "aulo.log.2"), listDir(dir).sorted())
        assertEquals("line-6xxx\n", readText(Path(dir, "aulo.log")))
        assertEquals("line-4xxx\nline-5xxx\n", readText(Path(dir, "aulo.log.1")))
        assertEquals("line-2xxx\nline-3xxx\n", readText(Path(dir, "aulo.log.2")))
    }

    @Test
    fun aLineLongerThanTheCapStillGetsWritten() {
        RotatingFileSink(dir, maxBytes = 4, keep = 0).use { sink ->
            sink.write("longer than four")
            sink.write("again, longer")
        }
        assertEquals("again, longer\n", readText(Path(dir, "aulo.log")))
        assertNull(entryKind(Path(dir, "aulo.log.1")))
    }

    @Test
    fun invalidLimitsAreRejected() {
        assertFailsWith<IllegalArgumentException> { RotatingFileSink(dir, maxBytes = 0) }
        assertFailsWith<IllegalArgumentException> { RotatingFileSink(dir, keep = -1) }
    }
}
