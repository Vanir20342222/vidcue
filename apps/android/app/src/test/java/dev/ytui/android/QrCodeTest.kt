package dev.ytui.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class QrCodeTest {
    @Test
    fun parsesSquareRowsOfZerosAndOnes() {
        val matrix = QrRows.parseRows(listOf("101", "010", "111"))
        assertNotNull(matrix)
        matrix!!
        assertEquals(3, matrix.size)
        assertTrue(matrix[0, 0])
        assertFalse(matrix[1, 0])
        assertTrue(matrix[1, 1])
        assertFalse(matrix[0, 1])
        assertTrue(matrix[2, 2])
        assertEquals(6, matrix.darkCount)
    }

    @Test
    fun refusesAnythingThatIsNotASquareBitMatrix() {
        assertNull(QrRows.parseRows(emptyList<String>()))
        assertNull("ragged", QrRows.parseRows(listOf("10", "1")))
        assertNull("not square", QrRows.parseRows(listOf("101", "010")))
        assertNull("other characters", QrRows.parseRows(listOf("1 ", "01")))
        assertNull("dark marks are 1, not #", QrRows.parseRows(listOf("#.", ".#")))
        val tooBig = List(QrRows.MAX_MODULES + 1) { "0".repeat(QrRows.MAX_MODULES + 1) }
        assertNull(QrRows.parseRows(tooBig))
        val largest = List(QrRows.MAX_MODULES) { "1".repeat(QrRows.MAX_MODULES) }
        assertEquals(QrRows.MAX_MODULES, QrRows.parseRows(largest)?.size)
    }

    @Test
    fun modulesAreWholePixelsAndTheCodeIsCentred() {
        // 29 modules (version 3 with its quiet zone) on a 300 px canvas: 10 px each, 5 px margins.
        assertEquals(QrLayout(10, 5f), QrRows.layout(300f, 29))
        assertEquals(QrLayout(1, 0f), QrRows.layout(20f, 29))
        val layout = QrRows.layout(1000f, 37)
        assertEquals(27, layout.module)
        assertEquals((1000f - 27 * 37) / 2f, layout.offset, 0.001f)
        assertTrue(layout.module * 37 + 2 * layout.offset <= 1000f + 0.001f)
    }
}
