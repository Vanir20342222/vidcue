package dev.ytui.android

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import java.util.Calendar
import java.util.TimeZone
import org.json.JSONObject

internal object StatsPeriods {
    val ALL = listOf("Week" to 7, "Month" to 30, "Year" to 365)

    /** Minutes east of UTC right now, bounded like the server's `tz_offset_min`. */
    fun tzOffsetMinutes(now: Long, zone: TimeZone = TimeZone.getDefault()): Int =
        (zone.getOffset(now) / 60_000).coerceIn(-840, 840)

    /** `[from, to)` for the last [days] days ending at [now]. */
    fun range(days: Int, now: Long): Pair<Long, Long> = (now - days * 86_400_000L) to now
}

/** Stats: totals as big numbers, top creators and items as artwork, plays by hour as bars. */
@Composable
internal fun StatsSheet(vm: YtuiViewModel, close: () -> Unit) {
    var days by remember { mutableIntStateOf(30) }
    var stats by remember { mutableStateOf<JSONObject?>(null) }
    var health by remember { mutableStateOf<JSONObject?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(days) {
        val now = System.currentTimeMillis()
        val (from, to) = StatsPeriods.range(days, now)
        error = null
        try {
            stats = vm.sync.obj("stats", "from" to from, "to" to to, "tz_offset_min" to StatsPeriods.tzOffsetMinutes(now))
        } catch (cancelled: kotlinx.coroutines.CancellationException) {
            throw cancelled
        } catch (failure: Exception) {
            error = SyncText.message(failure)
        }
        if (vm.sync.has("health")) health = runCatching { vm.sync.obj("health_summary", "days" to minOf(days, 90)) }.getOrNull()
    }
    Panel("Stats", close, page = true, actions = {
        Pill(Calendar.getInstance().get(Calendar.YEAR).toString(), Modifier.testTag("stats-wrapped"), PillStyle.Tonal) {
            vm.sync.open(SyncSheet.Wrapped)
        }
    }) {
        Row(Modifier.fillMaxWidth().testTag("stats-periods"), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            for ((label, value) in StatsPeriods.ALL)
                MediaChip(label, days == value, Modifier.testTag("stats-period-$value")) { days = value }
        }
        ErrorLine(error)
        val current = stats
        if (current == null) {
            if (error == null) Box(Modifier.fillMaxWidth().padding(32.dp), contentAlignment = Alignment.Center) { MediaSpinner() }
            return@Panel
        }
        Row(Modifier.fillMaxWidth().testTag("stats-totals"), horizontalArrangement = Arrangement.SpaceEvenly) {
            BigNumber(SyncText.hours(current.optDouble("seconds", 0.0)), "hours")
            BigNumber(current.optInt("plays").toString(), "plays")
            BigNumber(current.optInt("distinct_items").toString(), "items")
        }
        val creators = current.optJSONArray("top_creators")?.objects().orEmpty()
        if (creators.isNotEmpty()) {
            SectionHeader("Creators")
            LazyRow(Modifier.fillMaxWidth().testTag("stats-creators"), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                items(creators, key = { it.text("creator") }) { creator ->
                    Column(Modifier.width(88.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        val name = creator.text("creator")
                        Box(Modifier.size(80.dp)) {
                            ChannelAvatar(name, 80.dp, creator.text("thumbnail"), thumbnailsOn(vm))
                        }
                        Text(name, style = MaterialTheme.typography.labelMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(SyncText.hours(creator.optDouble("seconds", 0.0)) + " h", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }
        val top = current.optJSONArray("top_items")?.objects().orEmpty()
        if (top.isNotEmpty()) {
            SectionHeader("Most played")
            LazyRow(Modifier.fillMaxWidth().testTag("stats-items"), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                items(top, key = { it.text("url") }) { item ->
                    val card = MediaCard(item)
                    Column(Modifier.width(160.dp).clickable { vm.play(card) }, verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Artwork(card, Modifier.fillMaxWidth().aspectRatio(if (card.audio) 1f else 16f / 9f), thumbnailsOn(vm))
                        Text(card.title, style = MaterialTheme.typography.labelLarge, maxLines = 2, overflow = TextOverflow.Ellipsis)
                        Text(item.optInt("plays").toString() + " ×", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }
        val hours = current.optJSONArray("by_hour")
        if (hours != null && hours.length() == 24) {
            SectionHeader("By hour")
            HourBars((0 until 24).map { hours.optDouble(it, 0.0).toFloat() })
        }
        health?.let { HealthSummary(it) }
    }
}

/** Twenty-four bars, one per hour of the day, scaled to the busiest hour. */
@Composable
internal fun HourBars(values: List<Float>, modifier: Modifier = Modifier) {
    val ink = MaterialTheme.colorScheme.primary
    val rest = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.12f)
    val peak = values.maxOrNull()?.takeIf { it > 0f } ?: 1f
    val busiest = values.indices.maxByOrNull { values[it] } ?: -1
    Canvas(
        modifier.fillMaxWidth().height(96.dp).testTag("stats-hours").semantics {
            contentDescription = if (busiest >= 0 && values[busiest] > 0f) "Busiest hour $busiest:00" else "No plays"
        }
    ) {
        val gap = size.width / 24f * 0.25f
        val bar = size.width / 24f - gap
        values.forEachIndexed { hour, value ->
            val height = (value / peak).coerceIn(0f, 1f) * size.height
            drawRoundRect(rest, Offset(hour * (bar + gap), 0f), Size(bar, size.height), CornerRadius(bar / 2))
            if (height > 0f)
                drawRoundRect(ink, Offset(hour * (bar + gap), size.height - height), Size(bar, height), CornerRadius(bar / 2))
        }
    }
}

@Composable
private fun HealthSummary(summary: JSONObject) {
    val start = summary.optJSONObject("by_kind")?.optJSONObject("start") ?: return
    if (start.optInt("total") == 0) return
    SectionHeader("Playback")
    Row(Modifier.fillMaxWidth().testTag("stats-health"), horizontalArrangement = Arrangement.SpaceEvenly) {
        BigNumber(SyncText.percent(start.optDouble("ok_rate", Double.NaN)), "starts ok")
        val p50 = summary.optJSONObject("start_ms")?.optLong("p50") ?: 0L
        if (p50 > 0) BigNumber(String.format(java.util.Locale.ROOT, "%.1f", p50 / 1000.0), "s to start")
    }
}

/** The yearly review: one full-bleed page per highlight, swiped sideways. */
@Composable
internal fun WrappedSheet(vm: YtuiViewModel, close: () -> Unit) {
    val thisYear = Calendar.getInstance().get(Calendar.YEAR)
    var year by remember { mutableIntStateOf(thisYear) }
    var wrapped by remember { mutableStateOf<JSONObject?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(year) {
        wrapped = null
        error = null
        try {
            wrapped = vm.sync.obj("stats_wrapped", "year" to year, "tz_offset_min" to StatsPeriods.tzOffsetMinutes(System.currentTimeMillis()))
        } catch (cancelled: kotlinx.coroutines.CancellationException) {
            throw cancelled
        } catch (failure: Exception) {
            error = SyncText.message(failure)
        }
    }
    Panel(year.toString(), close, page = true, scroll = false, actions = {
        for (option in listOf(thisYear - 1, thisYear))
            MediaChip(option.toString(), option == year, Modifier.testTag("wrapped-year-$option")) { year = option }
    }) {
        ErrorLine(error)
        val current = wrapped ?: run {
            if (error == null) Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { MediaSpinner() }
            return@Panel
        }
        val pages = remember(current) { WrappedPages.of(current) }
        val pager = rememberPagerState { pages.size }
        HorizontalPager(pager, Modifier.weight(1f).fillMaxWidth().testTag("wrapped-pager"), pageSpacing = 12.dp) { index ->
            WrappedPage(vm, pages[index])
        }
        Row(Modifier.fillMaxWidth().padding(bottom = 8.dp), horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterHorizontally)) {
            repeat(pages.size) { index ->
                Box(
                    Modifier.size(if (index == pager.currentPage) 8.dp else 6.dp)
                        .background(
                            MaterialTheme.colorScheme.onBackground.copy(alpha = if (index == pager.currentPage) 0.9f else 0.3f),
                            CircleShape,
                        )
                )
            }
        }
    }
}

/** One Wrapped page: an artwork backdrop (optional), a big figure and a short caption. */
internal data class WrappedPage(val big: String, val caption: String, val detail: String = "", val artwork: JSONObject? = null)

internal object WrappedPages {
    fun of(w: JSONObject): List<WrappedPage> = buildList {
        add(WrappedPage(SyncText.hours(w.optDouble("seconds", 0.0)), "hours", "${w.optInt("plays")} plays"))
        w.optJSONArray("top_creators")?.optJSONObject(0)?.let {
            add(WrappedPage(it.text("creator"), "top creator", SyncText.hours(it.optDouble("seconds", 0.0)) + " h", it.takeIf { c -> c.text("thumbnail").isNotBlank() }))
        }
        w.optJSONArray("top_songs")?.optJSONObject(0)?.let { add(WrappedPage(it.text("title"), "top song", it.text("creator"), it)) }
        w.optJSONArray("top_videos")?.optJSONObject(0)?.let { add(WrappedPage(it.text("title"), "top video", it.text("creator"), it)) }
        w.optJSONObject("busiest_day")?.let { day ->
            if (day.text("day").isNotBlank()) add(WrappedPage(day.text("day"), "busiest day", SyncText.hours(day.optDouble("seconds", 0.0)) + " h"))
        }
        if (w.optInt("longest_streak_days") > 0) add(WrappedPage(w.optInt("longest_streak_days").toString(), "day streak"))
        if (w.optDouble("seconds", 0.0) > 0) add(WrappedPage(SyncText.percent(w.optDouble("music_share", 0.0)), "music", "${w.optInt("new_creators")} new creators"))
    }
}

@Composable
private fun WrappedPage(vm: YtuiViewModel, page: WrappedPage) {
    Box(Modifier.fillMaxSize().padding(horizontal = 16.dp).background(MaterialTheme.colorScheme.surfaceContainer, MediaTokens.card)) {
        val art = page.artwork
        if (art != null) {
            val card = if (art.has("url")) MediaCard(art) else artworkCard(page.big, art.text("thumbnail"), "channel")
            Artwork(card, Modifier.fillMaxSize(), thumbnailsOn(vm), MediaTokens.card)
            Box(
                Modifier.matchParentSize().background(
                    Brush.verticalGradient(listOf(Color.Transparent, Color.Black.copy(alpha = 0.82f))),
                    MediaTokens.card,
                )
            )
        }
        Column(
            Modifier.align(Alignment.BottomStart).padding(24.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            val ink = if (art != null) MediaTokens.videoInk else MaterialTheme.colorScheme.onSurface
            Text(page.caption, style = MaterialTheme.typography.titleMedium, color = ink.copy(alpha = 0.8f))
            Text(
                page.big,
                style = MaterialTheme.typography.displayMedium.copy(fontWeight = FontWeight.Bold),
                color = ink,
                maxLines = 3,
                overflow = TextOverflow.Ellipsis,
                textAlign = TextAlign.Start,
            )
            if (page.detail.isNotBlank()) Text(page.detail, style = MaterialTheme.typography.titleMedium, color = ink.copy(alpha = 0.8f))
        }
    }
}
