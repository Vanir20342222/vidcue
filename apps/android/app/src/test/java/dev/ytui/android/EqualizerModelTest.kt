package dev.ytui.android

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** Offline: the JSON the engine's "equalizer_presets" and "settings" operations return. */
@RunWith(RobolectricTestRunner::class)
class EqualizerModelTest {
    private val catalog =
        JSONObject()
            .put(
                "presets",
                JSONArray()
                    .put(JSONObject().put("name", "flat").put("gains", JSONArray(List(10) { 0.0 })))
                    .put(
                        JSONObject()
                            .put("name", "bass-boost")
                            .put("gains", JSONArray(listOf(6.0, 5.0, 4.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)))
                    ),
            )

    private fun config() = JSONObject().put("equalizer", "off").put("equalizer_custom", JSONObject())

    @Test
    fun presetsListOffBuiltinsThenSortedCustomCurves() {
        val config = config()
        config.getJSONObject("equalizer_custom")
            .put("zeta", JSONArray(List(10) { 1.0 }))
            .put("alpha", JSONArray(List(10) { -1.0 }))
        assertEquals(listOf("off", "flat", "bass-boost", "alpha", "zeta"), EqualizerModel.names(catalog, config))
        assertEquals("Bass Boost", EqualizerModel.label("bass-boost"))
        assertEquals("Off", EqualizerModel.label("off"))
        assertEquals(6f, EqualizerModel.gains(catalog, config, "bass-boost")[0])
        assertEquals(-1f, EqualizerModel.gains(catalog, config, "alpha")[9])
        assertEquals(List(10) { 0f }, EqualizerModel.gains(catalog, config, "off"))
        assertEquals("1 kHz", EqualizerModel.bandLabel(1000))
        assertEquals("+3 dB", EqualizerModel.gainLabel(3f))
        assertEquals("-12 dB", EqualizerModel.gainLabel(-12f))
        assertEquals("0 dB", EqualizerModel.gainLabel(0f))
    }

    @Test
    fun customCurvesAreCopiedEditedRenamedAndDeleted() {
        val draft = config()
        val first =
            EqualizerModel.createCustom(draft, catalog, EqualizerModel.gains(catalog, draft, "bass-boost"))
        assertEquals("custom", first)
        assertEquals("custom", EqualizerModel.selected(draft))
        assertTrue(EqualizerModel.isCustom(draft, "custom"))
        assertEquals(6f, EqualizerModel.gains(catalog, draft, "custom")[0])
        assertEquals("custom-2", EqualizerModel.createCustom(draft, catalog, List(10) { 0f }))

        EqualizerModel.setGain(draft, "custom", 9, 30f)
        assertEquals(12f, EqualizerModel.gains(catalog, draft, "custom")[9])
        EqualizerModel.setGain(draft, "custom", 1, -2.6f)
        assertEquals(-3f, EqualizerModel.gains(catalog, draft, "custom")[1])
        EqualizerModel.setGain(draft, "bass-boost", 0, 1f) // built-ins are read-only
        assertEquals(6f, EqualizerModel.gains(catalog, draft, "bass-boost")[0])

        EqualizerModel.select(draft, "custom")
        assertNull(EqualizerModel.nameError(catalog, draft, "custom", "night-drive"))
        assertNull(EqualizerModel.nameError(catalog, draft, "custom", "custom"))
        assertNotNull(EqualizerModel.nameError(catalog, draft, "custom", "bass-boost"))
        assertNotNull(EqualizerModel.nameError(catalog, draft, "custom", "off"))
        assertNotNull(EqualizerModel.nameError(catalog, draft, "custom", "custom-2"))
        assertNotNull(EqualizerModel.nameError(catalog, draft, "custom", "Night Drive"))
        assertNotNull(EqualizerModel.nameError(catalog, draft, "custom", ""))
        EqualizerModel.rename(draft, "custom", "night-drive")
        assertEquals("night-drive", EqualizerModel.selected(draft))
        assertFalse(EqualizerModel.isCustom(draft, "custom"))
        assertEquals(12f, EqualizerModel.gains(catalog, draft, "night-drive")[9])
        assertEquals("custom", EqualizerModel.freshName(catalog, draft))

        EqualizerModel.delete(draft, "night-drive")
        assertEquals("off", EqualizerModel.selected(draft))
        assertEquals(listOf("custom-2"), EqualizerModel.customNames(draft))
        EqualizerModel.delete(draft, "custom-2")
        assertEquals("off", EqualizerModel.selected(draft))
    }
}
