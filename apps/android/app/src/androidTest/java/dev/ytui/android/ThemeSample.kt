package dev.ytui.android

import org.json.JSONArray

/**
 * The themes the emulator drives through the UI. Every built-in palette is still checked for
 * contrast and presence by cheap palette tests; repainting a full screen per theme on a software
 * emulator is what made these suites slow, so the UI path covers the default dark and light
 * themes plus one other palette.
 */
internal fun representativeThemes(all: JSONArray): List<String> {
    val ids = (0 until all.length()).map { all.getString(it) }
    val picked = listOf("vidcue-dark", "vidcue-light", "catppuccin-latte").filter { it in ids }
    return picked.ifEmpty { ids.take(3) }
}
