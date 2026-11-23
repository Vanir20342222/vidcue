package dev.ytui.android

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import org.json.JSONArray

/** A square QR module matrix: `dark[y * size + x]`. */
internal class QrMatrix(val size: Int, private val dark: BooleanArray) {
    operator fun get(x: Int, y: Int): Boolean = dark[y * size + x]

    val darkCount: Int
        get() = dark.count { it }
}

/** Where modules land on a canvas: whole pixels per module, centred, so edges stay crisp. */
internal data class QrLayout(val module: Int, val offset: Float)

internal object QrRows {
    /** Version 40 is 177 modules; the engine adds a 4-module quiet zone on each side. */
    const val MAX_MODULES = 177 + 8

    /**
     * Parses the engine's rows of `0`/`1` (1 = dark). Anything that is not a square matrix of
     * those two characters is refused, so a malformed reply draws nothing rather than a code a
     * camera might misread.
     */
    fun parseRows(rows: List<String>): QrMatrix? {
        val size = rows.size
        if (size == 0 || size > MAX_MODULES) return null
        val dark = BooleanArray(size * size)
        rows.forEachIndexed { y, row ->
            if (row.length != size) return null
            row.forEachIndexed { x, c ->
                dark[y * size + x] =
                    when (c) {
                        '1' -> true
                        '0' -> false
                        else -> return null
                    }
            }
        }
        return QrMatrix(size, dark)
    }

    fun parseRows(rows: JSONArray?): QrMatrix? =
        if (rows == null || rows.length() > MAX_MODULES) null
        else parseRows((0 until rows.length()).map { rows.optString(it) })

    /**
     * Whole-pixel module size for a canvas [sidePx] wide, and the offset that centres the code.
     * Never zero: a canvas smaller than the matrix still draws one pixel per module.
     */
    fun layout(sidePx: Float, modules: Int): QrLayout {
        val module = maxOf(1, (sidePx / modules).toInt())
        return QrLayout(module, ((sidePx - module * modules) / 2f).coerceAtLeast(0f))
    }
}

/**
 * Draws [rows] as black modules on white, in light and dark themes alike: cameras need the dark
 * modules on a light quiet zone, which the engine's rows include.
 */
@Composable
internal fun QrCode(rows: List<String>, modifier: Modifier = Modifier, description: String = "QR code") {
    val matrix = remember(rows) { QrRows.parseRows(rows) } ?: return
    Canvas(
        modifier
            .widthIn(max = 320.dp)
            .fillMaxWidth()
            .aspectRatio(1f)
            .clip(MediaTokens.medium)
            .background(Color.White)
            .testTag("qr-code")
            .semantics { contentDescription = description }
    ) {
        val side = minOf(size.width, size.height)
        val layout = QrRows.layout(side, matrix.size)
        val cell = layout.module.toFloat()
        val origin = Offset(layout.offset + (size.width - side) / 2f, layout.offset + (size.height - side) / 2f)
        for (y in 0 until matrix.size) {
            var x = 0
            while (x < matrix.size) {
                if (!matrix[x, y]) {
                    x++
                    continue
                }
                // One rectangle per horizontal run keeps the draw list short.
                val start = x
                while (x < matrix.size && matrix[x, y]) x++
                drawRect(
                    Color.Black,
                    Offset(origin.x + start * cell, origin.y + y * cell),
                    Size((x - start) * cell, cell),
                )
            }
        }
    }
}

@Composable
internal fun QrCode(rows: JSONArray?, modifier: Modifier = Modifier, description: String = "QR code") {
    val list = remember(rows?.toString()) { rows?.let { r -> (0 until r.length()).map { r.optString(it) } } }
    if (list != null) QrCode(list, modifier, description)
}
