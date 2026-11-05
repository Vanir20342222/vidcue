package dev.ytui.android

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import org.json.JSONObject

/** Share of the line over which the sung colour fades into the unsung colour. */
internal const val LYRIC_SWEEP_EDGE = 0.06f

/** Alpha of lyric lines that are not sung yet, including the unsung part of the playing line. */
internal const val LYRIC_UNSUNG_ALPHA = 0.6f

/** Fill time of a line's last word when the lyrics give no closing time for it. */
internal const val LYRIC_LAST_WORD_MS = 600L

/** One timed word of an enhanced LRC line; the words joined in order are the line's text. */
internal data class LyricWord(val atMs: Long, val text: String)

/**
 * How far (0 to 1) the karaoke fill of the playing line has moved at [nowMs], or null when the
 * line has no word timings.
 *
 * Line-synced lyrics (plain LRC, YouTube Music) only time when each line starts, so the time to
 * the next line also holds instrumental breaks and says nothing about where the singer is. Like
 * YouTube Music, those lines are highlighted whole instead of guessing a fill. Enhanced LRC times
 * every word: the fill reaches each word when it is sung and crosses it until the next word starts.
 * The last word ends at [endMs] when the lyrics close the line, otherwise after
 * [LYRIC_LAST_WORD_MS], and never later than the next line at [nextLineMs].
 */
internal fun lyricSweep(nowMs: Double, words: List<LyricWord>, endMs: Long?, nextLineMs: Long?): Float? {
    if (words.isEmpty()) return null
    val lengths = words.map { it.text.codePointCount(0, it.text.length) }
    val total = lengths.sum()
    if (total == 0) return null
    val playing = words.indexOfLast { it.atMs <= nowMs }
    if (playing < 0) return 0f
    val word = words[playing]
    val limit = nextLineMs ?: Long.MAX_VALUE
    val wordEnd =
        (words.getOrNull(playing + 1)?.atMs ?: endMs ?: (word.atMs + LYRIC_LAST_WORD_MS))
            .coerceAtMost(limit)
    val within =
        if (wordEnd <= word.atMs) 1f
        else ((nowMs - word.atMs) / (wordEnd - word.atMs)).toFloat().coerceIn(0f, 1f)
    val before = lengths.subList(0, playing).sum()
    return ((before + within * lengths[playing]) / total).coerceIn(0f, 1f)
}

private fun lrcTime(ms: Long) = "%02d:%02d.%03d".format(ms / 60000, ms / 1000 % 60, ms % 1000)

/** A timed lyric line as LRC, keeping enhanced word tags so an edited copy keeps its word timing. */
internal fun lrcLine(line: JSONObject): String {
    val words = line.optJSONArray("words")?.objects().orEmpty()
    val body =
        if (words.isEmpty()) line.optString("text")
        else
            words.joinToString("") { "<${lrcTime(it.optLong("at_ms"))}>${it.optString("text")}" } +
                (if (line.isNull("end_ms")) "" else "<${lrcTime(line.optLong("end_ms"))}>")
    return "[${lrcTime(line.optLong("at_ms"))}]$body"
}

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
