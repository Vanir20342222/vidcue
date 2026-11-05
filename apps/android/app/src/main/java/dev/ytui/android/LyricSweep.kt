package dev.ytui.android

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import java.text.Normalizer

/** Share of the line over which the sung colour fades into the unsung colour. */
internal const val LYRIC_SWEEP_EDGE = 0.06f

/** Alpha of lyric lines that are not sung yet, including the unsung part of the playing line. */
internal const val LYRIC_UNSUNG_ALPHA = 0.6f

/** Estimated singing time per syllable when lyrics only time the start of each line. */
internal const val LYRIC_MS_PER_SYLLABLE = 300L

/** Shortest estimated singing time for a line, so one-word lines do not flash past. */
internal const val LYRIC_MIN_SUNG_MS = 700L

/** Sweep time for the last line, which has no next timestamp to bound it. */
internal const val LYRIC_LAST_LINE_MS = 4000L

private const val LATIN_VOWELS = "aeiouy"
private const val CYRILLIC_VOWELS = "аеёиоуыэюяіїє"
private const val GREEK_VOWELS = "αεηιουω"
private val WORD_BREAK = Regex("[^\\p{L}\\p{M}]+")

/**
 * Rough syllable count for [text]: vowel groups in alphabetic scripts, and one syllable per
 * character in scripts without vowel letters (CJK, kana, Hangul). Every word with letters counts
 * at least once.
 */
internal fun lyricSyllables(text: String): Int {
    val plain = Normalizer.normalize(text.lowercase(), Normalizer.Form.NFD)
    var total = 0
    for (word in plain.split(WORD_BREAK)) {
        if (word.isEmpty()) continue
        var count = 0
        var inVowel = false
        var offset = 0
        while (offset < word.length) {
            val cp = word.codePointAt(offset)
            offset += Character.charCount(cp)
            val type = Character.getType(cp)
            if (type == Character.NON_SPACING_MARK.toInt() || type == Character.COMBINING_SPACING_MARK.toInt())
                continue
            val script = Character.UnicodeScript.of(cp)
            val vowel =
                when (script) {
                    Character.UnicodeScript.LATIN -> LATIN_VOWELS.indexOf(cp.toChar()) >= 0
                    Character.UnicodeScript.CYRILLIC -> CYRILLIC_VOWELS.indexOf(cp.toChar()) >= 0
                    Character.UnicodeScript.GREEK -> GREEK_VOWELS.indexOf(cp.toChar()) >= 0
                    else -> {
                        if (Character.isLetter(cp)) count++
                        inVowel = false
                        continue
                    }
                }
            if (vowel && !inVowel) count++
            inVowel = vowel
        }
        total += count.coerceAtLeast(1)
    }
    return total
}

/**
 * How far (0 to 1) the karaoke fill of a line that starts at [atMs] has moved at [nowMs].
 *
 * Common lyric sources time only the start of each line, so the gap to [nextMs] also holds any
 * instrumental break after the words. Sweeping across that whole gap made the fill crawl behind
 * the singer. The fill instead follows an estimated singing pace for [text] and only uses the
 * full gap when the next line arrives sooner than that estimate.
 */
internal fun lyricSweep(nowMs: Double, atMs: Long, nextMs: Long?, text: String): Float {
    val sung =
        (lyricSyllables(text) * LYRIC_MS_PER_SYLLABLE).coerceAtLeast(LYRIC_MIN_SUNG_MS)
    val gap = (nextMs ?: (atMs + LYRIC_LAST_LINE_MS)) - atMs
    val span = minOf(sung, gap).coerceAtLeast(1)
    return ((nowMs - atMs) / span).toFloat().coerceIn(0f, 1f)
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
