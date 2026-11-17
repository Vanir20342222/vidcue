package dev.ytui.android

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyHorizontalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.layout
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import org.json.JSONObject

/** Songs show as rows, as in every music app; covers stay for albums, playlists and stations. */
internal fun MediaCard.isSongRow(tab: Int): Boolean =
    kind == "song" || (tab == 2 && !collection && kind != "station")

/** Title colour of the item that is playing: the accent, kept readable on the page. */
@Composable
private fun playingInk(current: Boolean): Color =
    if (current)
        mediaReadableText(MaterialTheme.colorScheme.primary, MaterialTheme.colorScheme.background)
    else Color.Unspecified

private fun durationOf(media: MediaCard): String =
    if (media.live) "Live"
    else if (media.value.isNull("duration")) ""
    else media.value.optLong("duration").takeIf { it > 0 }?.let { time(it.toFloat()) }.orEmpty()

private fun kindLabel(kind: String): String =
    when (kind) {
        "album" -> "Album"
        "playlist" -> "Playlist"
        "artist" -> "Artist"
        "channel" -> "Channel"
        "station" -> "Station"
        "song" -> "Song"
        else -> ""
    }

/**
 * A song as one compact row: small cover (or a track number on album pages), title, artist and
 * length. Tap plays it; press and hold or ⋮ opens its actions.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun SongRow(
    media: MediaCard,
    thumbnails: Boolean,
    showCreator: Boolean,
    current: Boolean,
    playing: Boolean,
    onPlay: () -> Unit,
    onMenu: () -> Unit,
    modifier: Modifier = Modifier,
    number: Int? = null,
    /** Quick picks leave the length out so titles have room. */
    showDuration: Boolean = true,
    /** Search results mix kinds, so each row says what it is ("Song · Nova Lane"). */
    showKind: Boolean = false,
    showMenu: Boolean = true,
) {
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 64.dp)
            .clip(MediaTokens.medium)
            .combinedClickable(
                onClickLabel = "Play",
                onLongClickLabel = "More options",
                onLongClick = onMenu,
                onClick = onPlay,
            )
            .padding(vertical = 6.dp)
            .testTag("media-card-${media.key}"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        if (number != null)
            Box(Modifier.width(32.dp), contentAlignment = Alignment.Center) {
                if (current) PlayingBars(playing, color = MaterialTheme.colorScheme.onBackground)
                else
                    Text(
                        "$number",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
            }
        else
            Box(Modifier.size(52.dp).clip(MediaTokens.small)) {
                Artwork(media, Modifier.fillMaxSize(), thumbnails, shape = MediaTokens.small)
                // The current song carries bars on its cover, still while paused, so it is marked
                // by more than the accent title's colour.
                if (current)
                    Box(
                        Modifier.matchParentSize().background(MediaTokens.videoScrim.copy(alpha = 0.45f)),
                        contentAlignment = Alignment.Center,
                    ) {
                        PlayingBars(playing)
                    }
            }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                media.displayTitle,
                style = MaterialTheme.typography.titleSmall,
                color = playingInk(current),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            val details =
                listOf(
                        if (!showKind && (media.kind == "song" || media.kind == "video")) "" else kindLabel(media.kind).ifBlank { "Video" },
                        if (showCreator) cleanByline(media.creator) else "",
                    )
                    .filter { it.isNotBlank() }
                    .joinToString(" · ")
            if (details.isNotBlank())
                Text(
                    details,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
        }
        durationOf(media).takeIf { showDuration && it.isNotBlank() }?.let {
            Text(
                it,
                // Tabular figures, so lengths line up down the list.
                style = MaterialTheme.typography.bodySmall.copy(fontFeatureSettings = "tnum"),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        // The dots, not their touch area, sit on the 16dp gutter: the same line as the header's icons.
        if (showMenu)
            Box(Modifier.offset(x = 12.dp)) {
                MediaIconButton(Glyph.More, "More options for ${media.title}", "media-menu-${media.key}", onMenu)
            }
    }
}

/**
 * A search result or playlist entry as one row: thumbnail on the left, title and details on the
 * right. Faster to scan than full-width cards. Channels and artists get a round picture.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun CompactMediaRow(
    media: MediaCard,
    thumbnails: Boolean,
    mobile: JSONObject,
    current: Boolean,
    playing: Boolean,
    onPlay: () -> Unit,
    onMenu: () -> Unit,
    modifier: Modifier = Modifier,
    inset: Dp = MediaTokens.textInset,
    /** Music results: small square or round art at the start, as in music apps. */
    music: Boolean = false,
    showKind: Boolean = true,
) {
    val person = media.kind == "artist" || media.kind == "channel"
    val square = media.audio || media.kind == "album" || (media.kind == "playlist" && media.musicSource)
    Row(
        modifier
            .fillMaxWidth()
            .clip(MediaTokens.medium)
            .combinedClickable(
                onClickLabel = if (media.collection) "Open" else "Play",
                onLongClickLabel = "More options",
                onLongClick = onMenu,
                onClick = onPlay,
            )
            .padding(start = inset, top = 6.dp, bottom = 6.dp)
            .testTag("media-card-${media.key}"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        val width =
            when {
                music && (person || square) -> 52.dp
                music -> 100.dp
                else -> minOf(176.dp, LocalConfiguration.current.screenWidthDp.dp * 0.42f)
            }
        Box(
            Modifier.width(width).aspectRatio(if (music && (person || square)) 1f else 16f / 9f),
            contentAlignment = Alignment.Center,
        ) {
            if (person)
                Artwork(media, Modifier.fillMaxHeight().aspectRatio(1f).clip(CircleShape), thumbnails, shape = CircleShape)
            else if (square)
                Artwork(media, Modifier.fillMaxHeight().aspectRatio(1f).clip(MediaTokens.small), thumbnails, shape = MediaTokens.small)
            else {
                Artwork(media, Modifier.fillMaxSize().clip(MediaTokens.medium), thumbnails, shape = MediaTokens.medium)
                durationOf(media).takeIf { it.isNotBlank() && mobile.optBoolean("show_duration", true) || media.live }?.let {
                    ArtworkBadge(media.live, it, Modifier.align(Alignment.BottomEnd).testTag("duration-${media.key}"), pulse = false)
                }
            }
            if (current) PlayingBars(playing, Modifier.align(Alignment.TopStart).padding(6.dp))
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Text(
                media.displayTitle,
                style = MaterialTheme.typography.titleSmall,
                color = playingInk(current),
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            val creator = if (mobile.optBoolean("show_channel", true)) media.creator else ""
            // Music bylines often start with the kind already ("Album • Fleetwood Mac").
            // The kind is left out where the page already says it: stations on Radio, albums
            // under an "Albums" heading.
            val kind = kindLabel(media.kind).takeIf { showKind && (media.collection || media.audio) && media.kind != "station" }.orEmpty()
            val byline =
                cleanByline(creator, keepKind = true).split(" · ")
                    .filter { it.isNotEmpty() && !it.equals(kindLabel(media.kind), true) }
            // Every station is live, so rows say what it plays and where from instead.
            val details =
                if (media.kind == "station") stationDetails(media)
                else (listOf(kind) + byline).filter { it.isNotBlank() }.joinToString(" · ")
            if (details.isNotBlank())
                Text(
                    details,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            if (!media.collection) CardInformation(media.value, mobile)
        }
        Box(Modifier.offset(x = 12.dp)) {
            MediaIconButton(Glyph.More, "More options for ${media.title}", "media-menu-${media.key}", onMenu)
        }
    }
}

/**
 * A station's genre and country ("Jazz · Germany"): its first tag that the name does not already
 * say, then its country.
 */
internal fun stationDetails(media: MediaCard): String {
    val genre =
        media.value.optString("description")
            .split(',')
            .map { it.trim() }
            .firstOrNull { it.isNotEmpty() && !media.title.contains(it, ignoreCase = true) }
            ?.split(' ')
            ?.joinToString(" ") { w -> w.replaceFirstChar(Char::titlecase) }
            .orEmpty()
    val country = cleanByline(media.creator).takeIf { !media.title.contains(it, ignoreCase = true) }.orEmpty()
    return listOf(genre, country).filter { it.isNotBlank() }.distinct().joinToString(" · ")
}

/**
 * Large station tiles for the top of Radio: square cover, name, then genre and country, the
 * same size as Music's shelf cards. A tap plays.
 */
@Composable
internal fun StationTile(
    media: MediaCard,
    thumbnails: Boolean,
    current: Boolean,
    playing: Boolean,
    onPlay: () -> Unit,
    onMenu: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier
            .combinedClickable(onClickLabel = "Play", onLongClickLabel = "More options", onLongClick = onMenu, onClick = onPlay)
            .testTag("station-tile-${media.key}"),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Box(Modifier.fillMaxWidth().aspectRatio(1f)) {
            Artwork(media, Modifier.fillMaxSize().clip(MediaTokens.medium), thumbnails, shape = MediaTokens.medium)
            if (current) PlayingBars(playing, Modifier.align(Alignment.TopStart).padding(8.dp))
        }
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                media.displayTitle,
                style = MaterialTheme.typography.titleSmall,
                color = playingInk(current),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                stationDetails(media),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

/** Section title with an optional labelled action at its end ("Play all"). */
@Composable
internal fun ShelfHeading(
    title: String,
    modifier: Modifier = Modifier,
    inset: Dp = MediaTokens.textInset,
    action: String? = null,
    actionTag: String = "",
    actionGlyph: Glyph? = Glyph.Play,
    onAction: () -> Unit = {},
) {
    // One height for every section heading, with or without an action, so the gap under each
    // heading is the same.
    Row(
        modifier.fillMaxWidth().padding(start = inset, end = if (inset > 0.dp) inset else 0.dp, top = 8.dp).heightIn(min = 32.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            title,
            Modifier.weight(1f).semantics { heading() },
            style = MaterialTheme.typography.titleLarge,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        if (action != null) {
            // A compact outlined button, as YouTube Music's "Play all": quieter than the heading.
            val colors = MaterialTheme.colorScheme
            // 32dp tall in the layout; its 48dp touch area overflows into the gaps around it.
            Box(Modifier.height(32.dp), contentAlignment = Alignment.Center) {
            Row(
                (if (actionTag.isNotEmpty()) Modifier.testTag(actionTag) else Modifier)
                    .requiredHeight(48.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                // A quiet text action, as Spotify's "Show all": lighter than the heading. Its
                // text, not its touch area, ends on the gutter.
                Row(
                    Modifier.height(32.dp)
                        .offset(x = 8.dp)
                        .clip(MediaTokens.pill)
                        .clickable(role = androidx.compose.ui.semantics.Role.Button, onClick = onAction)
                        .padding(horizontal = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    actionGlyph?.let { MediaIcon(it, Modifier.size(16.dp), colors.onSurfaceVariant) }
                    Text(action, style = MaterialTheme.typography.labelLarge, color = colors.onSurfaceVariant)
                }
            }
            }
        }
    }
}

/**
 * Songs in a shelf: four rows per column, paged sideways, like "Quick picks". Many more songs fit
 * on screen than with large covers, and each row stays easy to read.
 */
@Composable
internal fun QuickPicksShelf(
    cards: List<MediaCard>,
    thumbnails: Boolean,
    showCreator: Boolean,
    playingUrl: String?,
    playing: Boolean,
    onPlay: (MediaCard) -> Unit,
    onMenu: (MediaCard) -> Unit,
    inset: Dp = MediaTokens.textInset,
    modifier: Modifier = Modifier,
) {
    val rows = minOf(4, cards.size).coerceAtLeast(1)
    // The next page shows its covers and the start of its titles, so it reads as more to swipe to.
    val columnWidth = (LocalConfiguration.current.screenWidthDp.dp * 0.8f).coerceAtMost(420.dp)
    LazyHorizontalGrid(
        GridCells.Fixed(rows),
        modifier.fillMaxWidth().height(64.dp * rows),
        contentPadding = PaddingValues(horizontal = inset),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        items(cards, key = { it.key }) { media ->
            val current = media.url.isNotBlank() && media.url == playingUrl
            SongRow(
                media,
                thumbnails,
                showCreator,
                current,
                current && playing,
                onPlay = { onPlay(media) },
                onMenu = { onMenu(media) },
                showDuration = false,
                modifier = Modifier.width(columnWidth),
            )
        }
    }
}

/**
 * "Jump back in": up to six recent items as compact two-column tiles, the quickest way back to
 * what you were listening to.
 */
@Composable
internal fun RecentTiles(
    title: String,
    cards: List<MediaCard>,
    thumbnails: Boolean,
    inset: Dp = MediaTokens.textInset,
    onPlay: (MediaCard) -> Unit,
) {
    Column(
        Modifier.fillMaxWidth().testTag("recent-tiles"),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        ShelfHeading(title, inset = inset)
        Column(
            Modifier.padding(horizontal = inset),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            for (pair in cards.take(6).chunked(2))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    for (media in pair)
                        Row(
                            Modifier.weight(1f)
                                .height(56.dp)
                                .clip(MediaTokens.small)
                                .background(MaterialTheme.colorScheme.surfaceContainer)
                                .combinedClickable(onClickLabel = "Play", onClick = { onPlay(media) })
                                .testTag("recent-${media.key}"),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            Artwork(media, Modifier.size(56.dp), thumbnails, shape = MediaTokens.small)
                            Text(
                                media.displayTitle,
                                Modifier.weight(1f).padding(end = 8.dp),
                                style = MaterialTheme.typography.labelLarge,
                                maxLines = 2,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                    if (pair.size == 1) Spacer(Modifier.weight(1f))
                }
        }
    }
}

/** The page's focal point on Home: the last video you watched, ready to resume. */
@Composable
internal fun ContinueHero(media: MediaCard, thumbnails: Boolean, onResume: () -> Unit) {
    Box(
        Modifier.padding(horizontal = 12.dp)
            .fillMaxWidth()
            .aspectRatio(16f / 9f)
            .clip(MediaTokens.card)
            .combinedClickable(onClickLabel = "Resume", onClick = onResume)
            .testTag("continue-hero")
    ) {
        Artwork(media, Modifier.fillMaxSize(), thumbnails, shape = MediaTokens.card)
        Box(
            Modifier.matchParentSize()
                .background(
                    Brush.verticalGradient(
                        0f to Color.Transparent,
                        0.45f to Color.Transparent,
                        1f to Color.Black.copy(alpha = 0.82f),
                    )
                )
        )
        Column(
            Modifier.align(Alignment.BottomStart).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text(
                "Continue watching",
                style = MaterialTheme.typography.labelLarge,
                color = MediaTokens.videoInk.copy(alpha = 0.82f),
            )
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(
                    media.title,
                    Modifier.weight(1f),
                    style = MaterialTheme.typography.titleLarge,
                    color = MediaTokens.videoInk,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                Pill("Resume", Modifier.testTag("continue-resume"), PillStyle.Primary, glyph = Glyph.Play, onClick = onResume)
            }
        }
    }
}

/**
 * Header of a playlist, album, artist or channel page: cover, name, owner and count, with Play,
 * Shuffle, Download and Save. The cover's colour washes down behind it.
 */
@Composable
internal fun CollectionHeader(
    vm: YtuiViewModel,
    feed: FeedState,
    tab: Int,
    thumbnails: Boolean,
    inset: Dp = MediaTokens.textInset,
    modifier: Modifier = Modifier,
    onMore: (MediaCard) -> Unit = {},
) {
    val id = SubPages.collectionId(feed.request) ?: return
    val opened = vm.collectionCard(id)
    val cover = opened ?: feed.cards.firstOrNull()
    val title = opened?.title ?: feed.title
    val kind = opened?.kind ?: if (tab == 2) "album" else "playlist"
    val person = kind == "artist" || kind == "channel"
    val playable = feed.cards.filter { !it.collection && !it.live && it.url.isNotBlank() }
    val background = MaterialTheme.colorScheme.background
    val tint = rememberArtworkTint(cover, background, thumbnails)
    // The title row above continues the wash's top colour (see ambientGlow), so there is no seam.
    val top = androidx.compose.ui.graphics.lerp(background, tint, 0.85f * 0.8f)
    androidx.compose.runtime.SideEffect { PageChrome.headerTint.value = top }
    androidx.compose.runtime.DisposableEffect(Unit) { onDispose { PageChrome.headerTint.value = Color.Unspecified } }
    // Text and buttons line up with the covers in the rows below.
    val textStart = MediaTokens.textInset
    Column(
        modifier
            .fillMaxWidth()
            // Inside a padded grid (Music) the wash still reaches the screen edges, as one band
            // that fades into the page, not a card.
            .then(
                if (inset == 0.dp)
                    Modifier.layout { measurable, constraints ->
                        val bleed = MediaTokens.textInset.roundToPx()
                        val width = constraints.maxWidth + bleed * 2
                        val placeable = measurable.measure(constraints.copy(minWidth = width, maxWidth = width))
                        layout(constraints.maxWidth, placeable.height) { placeable.place(-bleed, 0) }
                    }
                else Modifier
            )
            .ambientGlow(tint, background, 0.8f)
            .padding(top = 8.dp, bottom = 4.dp)
            .testTag("collection-header"),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        if (cover != null) {
            val music = tab == 2 || cover.musicSource || cover.audio || kind == "album"
            val coverModifier =
                when {
                    person -> Modifier.size(148.dp).clip(CircleShape)
                    music -> Modifier.size(232.dp).clip(MediaTokens.medium)
                    else -> Modifier.fillMaxWidth(0.72f).aspectRatio(16f / 9f).clip(MediaTokens.medium)
                }
            // The cover measures itself; the grid's cell width does not apply to a header.
            androidx.compose.runtime.CompositionLocalProvider(LocalArtworkPixels provides 0) {
                Artwork(
                    cover,
                    coverModifier,
                    thumbnails,
                    shape = if (person) CircleShape else MediaTokens.medium,
                )
            }
        }
        Column(
            Modifier.fillMaxWidth().padding(horizontal = textStart),
            horizontalAlignment = Alignment.Start,
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            // The toolbar shows the name only once this title has scrolled away.
            androidx.compose.runtime.DisposableEffect(Unit) {
                PageChrome.headerTitleShown.value = true
                onDispose { PageChrome.headerTitleShown.value = false }
            }
            Text(
                title,
                Modifier.semantics { heading() }.onGloballyPositioned {
                    PageChrome.headerTitleShown.value = it.boundsInWindow().height > 8f
                },
                style = MaterialTheme.typography.headlineSmall,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            val count =
                when {
                    playable.isEmpty() -> ""
                    feed.continuation != null -> "${playable.size}+ ${if (tab == 2) "songs" else "videos"}"
                    playable.size == 1 -> "1 ${if (tab == 2) "song" else "video"}"
                    else -> "${playable.size} ${if (tab == 2) "songs" else "videos"}"
                }
            // Who made it on its own line, then what it is, how many and how long, as Spotify.
            val creator = cleanByline(opened?.creator.orEmpty()).takeUnless { person }.orEmpty()
            if (creator.isNotBlank())
                Text(
                    creator,
                    style = MaterialTheme.typography.titleSmall,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            val length =
                if (feed.continuation == null && playable.isNotEmpty() && playable.all { it.value.optDouble("duration", 0.0) > 0 })
                    totalLength(playable.sumOf { it.value.optDouble("duration", 0.0) }.toLong())
                else ""
            val line =
                listOf(kindLabel(kind), count, length)
                    .filter { it.isNotBlank() }
                    // A non-breaking space keeps each "·" with the word before it.
                    .joinToString("\u00A0· ")
            if (line.isNotBlank())
                Text(
                    line,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
        }
        Row(
            // Bare icon buttons have 12dp around their glyph; pull the row back so icons sit on the gutter.
            // The glyph, not its 48dp touch area, starts on the 16dp gutter.
            Modifier.fillMaxWidth().padding(start = textStart - 15.dp, end = textStart),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (opened != null && !person && !opened.live)
                RoundIconButton(
                    Glyph.DownloadCircle,
                    "Download ${opened.title}",
                    Modifier.testTag("collection-download"),
                ) {
                    DownloadFlight.launch(opened, null)
                    vm.play(opened, download = true)
                }
            if (opened != null)
                RoundIconButton(
                    Glyph.AddCircle,
                    "Save ${opened.title} to Library",
                    Modifier.testTag("collection-save"),
                ) {
                    vm.save(opened)
                }
            if (opened != null) MediaIconButton(Glyph.MoreHorizontal, "More options for ${opened.title}", "collection-more") { onMore(opened) }
            Spacer(Modifier.weight(1f))
            if (playable.size > 1)
                RoundIconButton(
                    Glyph.Shuffle,
                    "Shuffle play",
                    Modifier.testTag("collection-shuffle"),
                ) {
                    PlaybackSource.set("Playing from ${kindLabel(kind).lowercase()}", title)
                    vm.playAll(playable.shuffled())
                }
            if (playable.isNotEmpty())
                // A round accent play button, as Spotify's: the page's one primary action.
                RoundIconButton(
                    Glyph.Play,
                    "Play ${title}",
                    Modifier.testTag("collection-play"),
                    size = 56.dp,
                    iconSize = 28.dp,
                    container = MaterialTheme.colorScheme.primary,
                    tint = MaterialTheme.colorScheme.onPrimary,
                ) {
                    PlaybackSource.set("Playing from ${kindLabel(kind).lowercase()}", title)
                    vm.playAll(playable)
                }
        }
    }
}

/** A collection's running time, as music apps write it: "48 min", "1 hr 12 min". */
internal fun totalLength(seconds: Long): String {
    val minutes = ((seconds + 30) / 60).coerceAtLeast(1)
    return if (minutes < 60) "$minutes min"
    else if (minutes % 60 == 0L) "${minutes / 60} hr"
    else "${minutes / 60} hr ${minutes % 60} min"
}

/**
 * Signed-out Home has no recommendations to show. Instead of an empty page it offers topics to
 * start from and sign-in for a Home made for you.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun QuickStart(onTopic: (String) -> Unit, onSignIn: () -> Unit) {
    Column(
        Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 32.dp).testTag("quick-start"),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        GlyphBadge(Glyph.Search, 72.dp, MaterialTheme.colorScheme.onSurfaceVariant)
        Text(
            "Start with a topic",
            style = MaterialTheme.typography.headlineSmall,
            textAlign = TextAlign.Center,
        )
        Text(
            "Sign in for a Home made for you, or pick something to watch.",
            Modifier.widthIn(max = 360.dp),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
            verticalArrangement = Arrangement.spacedBy(0.dp),
        ) {
            for (topic in listOf("Music", "Gaming", "News", "Sports", "Podcasts", "Cooking", "Science", "Comedy"))
                MediaChip(topic, modifier = Modifier.testTag("topic-$topic")) { onTopic(topic) }
        }
        Action("Sign in", "feed-account-action", action = onSignIn)
    }
}

/**
 * Suggestions while typing: your earlier searches and titles you have played or are looking at
 * that contain what you typed, closest matches first.
 */
@Composable
internal fun TypingSuggestions(
    query: String,
    /** Your earlier searches, newest first. */
    recent: List<String>,
    /** Titles and names from your history and the page; artist lists are split into names. */
    names: List<String>,
    onPick: (String) -> Unit,
    onFill: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val needle = query.trim()
    val matches =
        remember(needle, recent, names) {
            if (needle.isEmpty()) emptyList()
            else {
                fun relevant(text: String) =
                    text.isNotEmpty() && text.contains(needle, ignoreCase = true) && !text.equals(needle, true)
                val searches = recent.map { it.trim() }.filter(::relevant).take(3).map { it to true }
                val found =
                    names
                        .flatMap(::suggestionPhrases)
                        .filter(::relevant)
                        .sortedBy { if (it.startsWith(needle, ignoreCase = true)) 0 else 1 }
                        .map { it to false }
                (searches + found).distinctBy { it.first.lowercase() }.take(8)
            }
        }
    androidx.compose.foundation.lazy.LazyColumn(
        modifier.background(MaterialTheme.colorScheme.background).testTag("typing-suggestions"),
        state = rememberLazyListState(),
        contentPadding = PaddingValues(vertical = 4.dp),
    ) {
        items(matches.size) { index ->
            val (text, searched) = matches[index]
            SuggestionRow(if (searched) Glyph.History else Glyph.Search, text, fill = { onFill(text) }) { onPick(text) }
        }
    }
}

/**
 * The parts of a title or byline worth suggesting: "UB40, Toto & Fleetwood Mac" offers each
 * name, and "Artist - Song (Official Video)" offers the artist and the song.
 */
internal fun suggestionPhrases(text: String): List<String> {
    val clean = text.replace(Regex("""\s*[(\[][^)\]]*[)\]]"""), "").trim()
    val parts =
        clean.split(Regex("""\s*(?:,|&|•|·|\s-\s|\s–\s|\sx\s|\bfeat\.?|\bft\.?)\s*""", RegexOption.IGNORE_CASE))
            .map { it.trim() }
            .filter { it.length >= 2 && !it.matches(Regex("""[\d.,]+[KMB]? (?:plays|views|songs)""", RegexOption.IGNORE_CASE)) }
    // The whole text is a phrase too when it is short and not a list ("Simon & Garfunkel").
    val whole = listOf(clean).filter { it.length <= 40 && it.none { c -> c in ",•·" } }
    return (whole + parts).distinct()
}

@Composable
private fun SuggestionRow(
    glyph: Glyph,
    text: String,
    modifier: Modifier = Modifier,
    fill: (() -> Unit)? = null,
    pick: () -> Unit,
) {
    Row(
        modifier.fillMaxWidth()
            .heightIn(min = 52.dp)
            .combinedClickableCompat(pick)
            .padding(start = MediaTokens.textInset, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        MediaIcon(glyph, Modifier.size(20.dp), MaterialTheme.colorScheme.onSurfaceVariant)
        Text(
            text,
            Modifier.weight(1f),
            style = MaterialTheme.typography.bodyLarge,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        if (fill != null) MediaIconButton(Glyph.FillQuery, "Edit “$text” before searching", action = fill)
    }
}

@OptIn(ExperimentalFoundationApi::class)
private fun Modifier.combinedClickableCompat(onClick: () -> Unit): Modifier =
    this.combinedClickable(onClick = onClick)

/** Height that keeps [rows] song rows readable; used to size shelves before they compose. */
internal fun songShelfHeight(rows: Int): Dp = 64.dp * rows + 4.dp * (rows - 1)

/** Text style weight for numbers in the header line; kept here so rows and header agree. */
internal val CountWeight = FontWeight.Medium

/**
 * The best match of a music search, larger than the rows under it: a round artist picture (or a
 * square cover), the name, and what it is.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun TopResultCard(
    media: MediaCard,
    thumbnails: Boolean,
    inset: Dp,
    onOpen: () -> Unit,
    onMenu: () -> Unit,
) {
    val person = media.kind == "artist" || media.kind == "channel"
    val shape = if (person) CircleShape else MediaTokens.medium
    Row(
        Modifier.fillMaxWidth()
            .padding(horizontal = inset)
            .clip(MediaTokens.card)
            .background(MaterialTheme.colorScheme.surfaceContainer)
            .combinedClickable(onClickLabel = "Open", onLongClickLabel = "More options", onLongClick = onMenu, onClick = onOpen)
            .padding(16.dp)
            .testTag("top-result"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        androidx.compose.runtime.CompositionLocalProvider(LocalArtworkPixels provides 0) {
            Artwork(media, Modifier.size(88.dp).clip(shape), thumbnails, shape = shape)
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(media.title, style = MaterialTheme.typography.titleLarge, maxLines = 2, overflow = TextOverflow.Ellipsis)
            Text(
                cleanByline(media.creator, keepKind = true).ifBlank { kindLabel(media.kind) },
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

/** The music search group a result kind is listed under on "All". */
internal fun searchGroupOf(kind: String): String =
    when (kind) {
        "artist", "channel" -> "Artists"
        "song" -> "Songs"
        "album" -> "Albums"
        "playlist" -> "Playlists"
        else -> "Videos"
    }
