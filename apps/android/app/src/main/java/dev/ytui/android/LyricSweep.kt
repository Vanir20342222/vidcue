package dev.ytui.android

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle

/** Share of the line over which the sung colour fades into the unsung colour. */
internal const val LYRIC_SWEEP_EDGE = 0.06f

/** Alpha of lyric lines that are not sung yet, including the unsung part of the playing line. */
internal const val LYRIC_UNSUNG_ALPHA = 0.6f

/**
 * How far the character at [index] of [count] is sung when the playing line is [sweep] done
 * (0 to 1). The sweep runs through the characters in reading order, so text that wraps onto a
 * second visual line keeps that line unsung until the first one is finished.
 */
internal fun lyricSungFraction(index: Int, count: Int, sweep: Float): Float {
    if (count <= 0) return 0f
    val centre = (index + 0.5f) / count
    return ((sweep - centre) / LYRIC_SWEEP_EDGE + 0.5f).coerceIn(0f, 1f)
}

/**
 * The playing line with a karaoke fill. Each code point gets [sung] or [unsung] colour by
 * [lyricSungFraction], so a single fill moves through the whole lyric instead of through the
 * width of each visual line. Code points are kept whole, so emoji and other surrogate pairs
 * never split across colour spans.
 */
internal fun karaokeLine(
    text: String,
    sweep: Float,
    sung: Color,
    unsung: Color,
): AnnotatedString {
    val count = text.codePointCount(0, text.length)
    return buildAnnotatedString {
        var offset = 0
        var index = 0
        while (offset < text.length) {
            val next = text.offsetByCodePoints(offset, 1)
            val colour = lerp(unsung, sung, lyricSungFraction(index, count, sweep))
            withStyle(SpanStyle(color = colour)) { append(text.substring(offset, next)) }
            offset = next
            index++
        }
    }
}
