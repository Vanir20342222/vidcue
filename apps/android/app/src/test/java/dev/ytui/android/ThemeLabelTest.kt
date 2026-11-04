package dev.ytui.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

class ThemeLabelTest {
    @Test
    fun themeIdsBecomeReadableNames() {
        assertEquals("ytui Dark", themeLabel("ytui-dark"))
        assertEquals("Catppuccin Mocha", themeLabel("catppuccin-mocha"))
        assertEquals("System", themeLabel("system"))
        assertEquals("Gruvbox", themeLabel("gruvbox"))
        assertEquals("Nothing Dark", themeLabel("nothing-dark"))
        assertEquals("Matte Black", themeLabel("matte-black"))
        assertEquals("Flexoki Light", themeLabel("flexoki-light"))
        assertEquals("", themeLabel(""))
    }

    @Test
    fun onlyBuiltInNothingPalettesSwitchToDotMatrixType() {
        assertTrue(YtuiType.isDotMatrix("nothing-dark", "Built-in"))
        assertTrue(YtuiType.isDotMatrix("nothing-light", "Built-in"))
        assertFalse(YtuiType.isDotMatrix("ytui-dark", "Built-in"))
        // An imported palette that reuses the name keeps the default type.
        assertFalse(YtuiType.isDotMatrix("nothing-dark", "Omarchy"))
        assertSame(YtuiType.typography, YtuiType.forPalette(null))
    }

    @Test
    fun nothingTypeUsesDotMatrixHeadlinesAndReadableBody() {
        val nothing = YtuiType.nothing
        assertEquals(YtuiType.dotMatrix, nothing.headlineMedium.fontFamily)
        assertEquals(YtuiType.dotMatrix, nothing.displaySmall.fontFamily)
        assertEquals(YtuiType.geist, nothing.bodyLarge.fontFamily)
        assertEquals(YtuiType.geistMono, nothing.labelSmall.fontFamily)
        // Sizes stay the shared scale so layouts do not shift between themes.
        assertEquals(YtuiType.typography.headlineMedium.fontSize, nothing.headlineMedium.fontSize)
        assertNotEquals(YtuiType.typography.headlineMedium.fontFamily, nothing.headlineMedium.fontFamily)
    }
}
