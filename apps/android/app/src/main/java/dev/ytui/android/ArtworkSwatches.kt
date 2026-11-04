package dev.ytui.android

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.Color
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

private const val HUE_BINS = 18
private const val MAX_SWATCHES = 3

/**
 * The cover's own colours for the visualisers: up to three dominant hues, each the average of
 * the pixels that carry it, so a green-and-orange cover gives green and orange rather than the
 * brown its plain average would. Grey, near-black and near-white pixels count only when the
 * cover has nothing else, so a monochrome cover gives its own light grey. Colours are lifted
 * just enough to read on the dark player while keeping their hue.
 *
 * [pixels] are ARGB ints. Returns colours from most to least dominant; empty for no pixels.
 */
internal fun artworkSwatches(pixels: IntArray): List<Color> {
    val weight = FloatArray(HUE_BINS)
    val red = FloatArray(HUE_BINS)
    val green = FloatArray(HUE_BINS)
    val blue = FloatArray(HUE_BINS)
    var neutralWeight = 0f
    var neutralR = 0f
    var neutralG = 0f
    var neutralB = 0f
    val hsv = FloatArray(3)
    for (pixel in pixels) {
        val r = (pixel shr 16 and 255) / 255f
        val g = (pixel shr 8 and 255) / 255f
        val b = (pixel and 255) / 255f
        rgbToHsv(r, g, b, hsv)
        val saturation = hsv[1]
        val value = hsv[2]
        if (saturation < 0.18f || value < 0.16f) {
            val w = 0.2f + value
            neutralWeight += w
            neutralR += r * w
            neutralG += g * w
            neutralB += b * w
            continue
        }
        // Vivid, bright pixels say most about a cover's colour.
        val w = saturation * saturation * (0.3f + value)
        val bin = ((hsv[0] / 360f) * HUE_BINS).toInt().coerceIn(0, HUE_BINS - 1)
        weight[bin] += w
        red[bin] += r * w
        green[bin] += g * w
        blue[bin] += b * w
    }
    val total = weight.sum()
    val picked = mutableListOf<Int>()
    if (total > 0f) {
        // Merge each bin with its neighbours, then take peaks at least three bins (60°) apart.
        val smoothed = FloatArray(HUE_BINS) { i ->
            weight[i] + 0.5f * (weight[(i + 1) % HUE_BINS] + weight[(i + HUE_BINS - 1) % HUE_BINS])
        }
        val order = (0 until HUE_BINS).sortedByDescending { smoothed[it] }
        for (bin in order) {
            if (picked.size == MAX_SWATCHES) break
            if (weight[bin] < total * 0.06f) continue
            val distance = picked.minOfOrNull { other ->
                minOf((bin - other + HUE_BINS) % HUE_BINS, (other - bin + HUE_BINS) % HUE_BINS)
            } ?: HUE_BINS
            if (distance >= 3) picked += bin
        }
    }
    val colours =
        picked.map { bin ->
            liftForDarkStage(red[bin] / weight[bin], green[bin] / weight[bin], blue[bin] / weight[bin])
        }
    if (colours.isNotEmpty() || neutralWeight == 0f) return colours
    return listOf(
        liftForDarkStage(neutralR / neutralWeight, neutralG / neutralWeight, neutralB / neutralWeight)
    )
}

/** Keeps the hue, gives modest saturation a little more punch and raises the brightness. */
private fun liftForDarkStage(r: Float, g: Float, b: Float): Color {
    val hsv = FloatArray(3)
    rgbToHsv(r, g, b, hsv)
    if (hsv[1] >= 0.18f) hsv[1] = (hsv[1] * 1.15f).coerceIn(0.35f, 0.9f)
    hsv[2] = hsv[2].coerceIn(0.82f, 1f)
    return Color.hsv(hsv[0], hsv[1], hsv[2])
}

private fun rgbToHsv(r: Float, g: Float, b: Float, out: FloatArray) {
    val max = maxOf(r, g, b)
    val min = minOf(r, g, b)
    val delta = max - min
    val hue =
        when {
            delta == 0f -> 0f
            max == r -> 60f * (((g - b) / delta) % 6f)
            max == g -> 60f * (((b - r) / delta) + 2f)
            else -> 60f * (((r - g) / delta) + 4f)
        }
    out[0] = if (hue < 0f) hue + 360f else hue
    out[1] = if (max == 0f) 0f else delta / max
    out[2] = max
}

/** Lazily created, so the pure extraction above stays usable in JVM unit tests. */
private object SwatchCache {
    val cache = android.util.LruCache<String, List<Color>>(48)
}

/** The cover's swatches once its small artwork has loaded; null until then or without art. */
@Composable
internal fun rememberArtworkSwatches(media: MediaCard?, enabled: Boolean): List<Color>? {
    var swatches by remember(media?.key) { mutableStateOf(media?.let { SwatchCache.cache.get(it.key) }) }
    LaunchedEffect(media?.key, media?.artworkSource, enabled) {
        if (media == null || !enabled || swatches != null) return@LaunchedEffect
        val bitmap =
            ArtworkCache.cached(media, 96) ?: runCatching { ArtworkCache.load(media, 96) }.getOrNull()
                ?: return@LaunchedEffect
        val found =
            withContext(Dispatchers.Default) {
                val pixels = IntArray(bitmap.width * bitmap.height)
                bitmap.getPixels(pixels, 0, bitmap.width, 0, 0, bitmap.width, bitmap.height)
                artworkSwatches(pixels)
            }
        if (found.isNotEmpty()) {
            SwatchCache.cache.put(media.key, found)
            swatches = found
        }
    }
    return swatches
}
