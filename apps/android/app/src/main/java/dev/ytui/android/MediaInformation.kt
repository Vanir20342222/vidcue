package dev.ytui.android

import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.foundation.layout.size
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import org.json.JSONObject

internal fun mediaInformation(media: JSONObject, settings: JSONObject, includeAge: Boolean = true): String {
    val details = mutableListOf<String>()
    // Off unless chosen in Card details: cards stay lean (channel and age).
    if (settings.optBoolean("show_views", false)) {
        val count = media.optLong("view_count", -1)
        val views =
            if (count >= 0) "${compactCount(count)} ${if (count == 1L) "view" else "views"}"
            else media.optString("views_text").takeUnless { it == "null" }?.let(::compactViewsText) ?: ""
        if (views.isNotBlank()) details += views
    }
    if (includeAge && settings.optBoolean("show_upload_date", false)) {
        val text = uploadAge(media)
        if (text.isNotBlank()) details += text
    }
    if (settings.optBoolean("show_likes") && media.optLong("like_count", -1) >= 0)
        details +=
            "${compactCount(media.optLong("like_count"))} likes"
    return details.joinToString(" · ")
}

/**
 * How long ago [media] was uploaded: "3 hours ago", "5 years ago". The provider's own text is
 * finer than a day, so it wins over an upload date of today; blank when neither is known.
 */
internal fun uploadAge(media: JSONObject): String {
    val published = media.optString("published_text").takeUnless { it == "null" }.orEmpty().trim()
    val date = media.optString("upload_date").takeIf { it.matches(UPLOAD_DATE) }?.let(::relativeUploadDate)
    return when {
        date != null && (date != "Today" || published.isBlank()) -> date
        published.isNotBlank() -> spelledAge(published)
        else -> ""
    }
}

/** Reuse details learned during playback on matching Home/search cards. */
internal fun withMediaInformation(card: JSONObject, details: JSONObject): JSONObject {
    if (card.optString("url").isBlank() || card.optString("url") != details.optString("url")) return card
    var merged: JSONObject? = null
    for (key in listOf("view_count", "views_text", "upload_date", "published_text", "like_count", "duration", "description", "channel_follower_count", "subscribers_text")) {
        val value = details.opt(key)
        if (value == null || value == JSONObject.NULL || value.toString().isBlank() || value == "null") continue
        if (value is Number && value.toDouble() < 0) continue
        if (card.opt(key) == value) continue
        if (merged == null) merged = card.copyJson()
        merged.put(key, value)
    }
    return merged ?: card
}

/** Card preferences only control browsing. The watch page always exposes known details. */
internal fun playerInformation(media: JSONObject): String =
    // The player shows every available detail, whatever the card settings (whose views and
    // dates are off by default).
    mediaInformation(
        media,
        JSONObject().put("show_views", true).put("show_upload_date", true).put("show_likes", true),
    ).let { counts ->
        listOf(counts, mediaLength(media)).filter { it.isNotBlank() }.joinToString(" · ")
    }

private fun mediaLength(media: JSONObject): String =
    if (media.optBoolean("live")) "Live"
    else media.optDouble("duration", 0.0).takeIf { it.isFinite() && it > 0 }
        ?.let { time(it.toFloat()) }.orEmpty()

/** "283K", "1.2M": the short counts a watch page shows instead of every digit. */
internal fun compactCount(count: Long): String {
    val digits = if (count in 1_000..9_999 || count in 1_000_000..9_999_999) 1 else 0
    // The surrounding labels are English, so counts are too: a German device would otherwise
    // print "2834 likes" (ICU has no short thousands form there) next to "1.2K views".
    val locale = java.util.Locale.ENGLISH
    // Building an ICU formatter loads locale data; doing it per card cost scroll frames.
    // Formatters are kept per locale and digit count (and are not shared across threads).
    val format =
        synchronized(COMPACT_FORMATS) {
            COMPACT_FORMATS.getOrPut(locale to digits) {
                android.icu.text.CompactDecimalFormat.getInstance(
                        locale,
                        android.icu.text.CompactDecimalFormat.CompactStyle.SHORT,
                    )
                    .apply {
                        // "283K", not "284K": like YouTube, never round a count up.
                        roundingMode = android.icu.math.BigDecimal.ROUND_DOWN
                        maximumFractionDigits = digits
                    }
            }
        }
    return synchronized(format) { format.format(count) }
}

private val COMPACT_FORMATS = HashMap<Pair<java.util.Locale, Int>, android.icu.text.CompactDecimalFormat>()
private val UPLOAD_DATE = Regex("[0-9]{8}")

/** `yyyyMMdd` as "3 days ago" / "7 months ago"; falls back to the ISO date when parsing fails. */
/** "11,635,532 views" as YouTube writes it on cards: "11M views". Other text is kept. */
internal fun compactViewsText(text: String): String {
    val match = Regex("""^([\d,.\u00A0\u202F ]+)\s+(views?)$""", RegexOption.IGNORE_CASE).find(text.trim()) ?: return text
    val digits = match.groupValues[1].filter(Char::isDigit)
    val count = digits.toLongOrNull() ?: return text
    return "${compactCount(count)} ${if (count == 1L) "view" else "views"}"
}

/** "7d ago" and "3mo ago" spelled out ("7 days ago"), as cards write ages elsewhere. */
internal fun spelledAge(text: String): String {
    val match = Regex("""^(\d+)\s*(s|m|min|h|d|w|mo|y)\s+ago$""").find(text.trim()) ?: return text
    val n = match.groupValues[1].toInt()
    val unit =
        when (match.groupValues[2]) {
            "s" -> "second"
            "m", "min" -> "minute"
            "h" -> "hour"
            "d" -> "day"
            "w" -> "week"
            "mo" -> "month"
            else -> "year"
        }
    return "$n $unit${if (n == 1) "" else "s"} ago"
}

internal fun relativeUploadDate(date: String, today: java.time.LocalDate = java.time.LocalDate.now()): String {
    val iso = "${date.substring(0, 4)}-${date.substring(4, 6)}-${date.substring(6, 8)}"
    val day = runCatching { java.time.LocalDate.parse(iso) }.getOrNull() ?: return iso
    val period = java.time.Period.between(day, today)
    fun ago(n: Int, unit: String) = "$n $unit${if (n == 1) "" else "s"} ago"
    return when {
        period.isNegative -> iso
        period.years > 0 -> ago(period.years, "year")
        period.months > 0 -> ago(period.months, "month")
        period.days >= 7 -> ago(period.days / 7, "week")
        period.days > 0 -> ago(period.days, "day")
        else -> "Today"
    }
}

@Composable
internal fun MediaInformation(
    media: JSONObject,
    settings: JSONObject,
    /** Shown first on the same line, as YouTube does: "Channel · 240K views · 6 days ago". */
    lead: String = "",
    leadModifier: Modifier = Modifier,
    includeAge: Boolean = true,
) {
    val text = listOf(lead, mediaInformation(media, settings, includeAge)).filter { it.isNotBlank() }.joinToString(" · ")
    if (text.isNotBlank())
        Text(
            text,
            (if (lead.isNotBlank()) leadModifier else Modifier).testTag("video-information"),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 2,
            overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
        )
}

/** Shared by feed, shelf and wide search cards so each setting has the same effect. */
@Composable
internal fun CardInformation(
    media: JSONObject,
    settings: JSONObject,
    lead: String = "",
    leadModifier: Modifier = Modifier,
) {
    // The age gets its own line with a clock, so it reads at a glance under the channel.
    MediaInformation(media, settings, lead, leadModifier, includeAge = false)
    val age = if (settings.optBoolean("show_upload_date", false)) uploadAge(media) else ""
    if (age.isNotBlank())
        androidx.compose.foundation.layout.Row(
            Modifier.testTag("card-age"),
            verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
            horizontalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(4.dp),
        ) {
            val ink = MaterialTheme.colorScheme.onSurfaceVariant
            MediaIcon(Glyph.Clock, Modifier.size(12.dp), ink)
            Text(age, style = MaterialTheme.typography.bodySmall, color = ink, maxLines = 1)
        }
    for ((key, field) in listOf("show_description" to "description", "show_section" to "section")) {
        val text = media.optString(field).takeUnless { it == "null" }.orEmpty()
        if (settings.optBoolean(key) && text.isNotBlank())
            Text(
                text,
                Modifier.testTag("card-$field"),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = if (field == "description") 2 else 1,
                overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
            )
    }
}
