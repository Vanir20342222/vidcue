package dev.ytui.android

import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import org.json.JSONObject

internal fun mediaInformation(media: JSONObject, settings: JSONObject): String {
    val details = mutableListOf<String>()
    if (settings.optBoolean("show_views", true)) {
        val count = media.optLong("view_count", -1)
        val views =
            if (count >= 0) "${compactCount(count)} ${if (count == 1L) "view" else "views"}"
            else media.optString("views_text").takeUnless { it == "null" } ?: ""
        if (views.isNotBlank()) details += views
    }
    if (settings.optBoolean("show_upload_date", true)) {
        val date = media.optString("upload_date").takeIf { it.matches(UPLOAD_DATE) }
        val text =
            date?.let(::relativeUploadDate)
                ?: media.optString("published_text").takeUnless { it == "null" }
                ?: ""
        if (text.isNotBlank()) details += text
    }
    if (settings.optBoolean("show_likes") && media.optLong("like_count", -1) >= 0)
        details +=
            "${compactCount(media.optLong("like_count"))} likes"
    return details.joinToString(" · ")
}

/** "283K", "1.2M": the short counts a watch page shows instead of every digit. */
internal fun compactCount(count: Long): String {
    val digits = if (count in 1_000..9_999 || count in 1_000_000..9_999_999) 1 else 0
    // Building an ICU formatter loads locale data; doing it per card cost scroll frames.
    // Formatters are kept per locale and digit count (and are not shared across threads).
    val format =
        synchronized(COMPACT_FORMATS) {
            COMPACT_FORMATS.getOrPut(java.util.Locale.getDefault() to digits) {
                android.icu.text.CompactDecimalFormat.getInstance(
                        java.util.Locale.getDefault(),
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
internal fun MediaInformation(media: JSONObject, settings: JSONObject) {
    val text = mediaInformation(media, settings)
    if (text.isNotBlank())
        Text(
            text,
            Modifier.testTag("video-information"),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 2,
            overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
        )
}
