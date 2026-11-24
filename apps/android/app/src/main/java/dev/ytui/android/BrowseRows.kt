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
import androidx.compose.foundation.lazy.items as listItems
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
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
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
    /** Stations: a trailing save button, so Radio's "Saved" can be filled from the list. */
    onSave: (() -> Unit)? = null,
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
            // Every station is live, so rows say what it plays instead.
            val details =
                if (media.kind == "station") stationGenre(media)
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
        // Stations open their menu on press and hold, like Quick picks, and keep a save button.
        if (media.kind == "station" && onSave != null)
            Box(Modifier.offset(x = 12.dp)) {
                // Grey and a size down, as the ⋮ in other lists: quieter than the names.
                RoundIconButton(
                    Glyph.AddCircle,
                    "Save ${media.title}",
                    Modifier.testTag("station-save-${media.key}"),
                    iconSize = 20.dp,
                    tint = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.75f),
                    action = onSave,
                )
            }
        else if (media.kind != "station")
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

/** Large covers in a row (Jump back in, Featured stations): two whole and a slice of a third. */
@Composable
internal fun heroCoverSize(): Dp = ((LocalConfiguration.current.screenWidthDp.dp - 16.dp) / 2.25f).coerceAtMost(148.dp)

/** One word for a station row: its genre, else its country. */
internal fun stationGenre(media: MediaCard): String = stationDetails(media).substringBefore(" · ")

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
        Box(Modifier.fillMaxWidth().aspectRatio(1f).clip(MediaTokens.medium)) {
            Artwork(media, Modifier.fillMaxSize(), thumbnails, shape = MediaTokens.medium)
            // The name on the cover over a soft shade, the same as Music's large covers.
            Box(
                Modifier.fillMaxSize().background(
                    androidx.compose.ui.graphics.Brush.verticalGradient(
                        0.45f to Color.Transparent,
                        1f to Color.Black.copy(alpha = 0.6f),
                    )
                )
            )
            Text(
                media.displayTitle,
                Modifier.align(Alignment.BottomStart).padding(10.dp),
                style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.Bold),
                color = if (current) mediaReadableText(MaterialTheme.colorScheme.primary, Color.Black) else Color.White,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            if (current) PlayingBars(playing, Modifier.align(Alignment.TopStart).padding(8.dp))
        }
    }
}

/** Section title with an optional action at its end: › for "Show all", ▶ for "Play all". */
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
            // Icons instead of words, as YouTube Music's chevron: › opens the section, a round
            // play button plays it. The action's words stay its accessible name.
            val colors = MaterialTheme.colorScheme
            val glyph =
                when {
                    action == "Show less" -> Glyph.Up
                    actionGlyph == Glyph.Play -> Glyph.Play
                    else -> Glyph.Forward
                }
            val play = glyph == Glyph.Play
            // 32dp tall in the layout; its 48dp touch area overflows into the gaps around it. The
            // glyph, not the touch area, ends on the gutter.
            Box(Modifier.height(32.dp), contentAlignment = Alignment.Center) {
                Box(
                    (if (actionTag.isNotEmpty()) Modifier.testTag(actionTag) else Modifier)
                        .offset(x = if (play) 4.dp else 12.dp)
                        .requiredSize(48.dp)
                        .clip(CircleShape)
                        .clickable(role = androidx.compose.ui.semantics.Role.Button, onClickLabel = action, onClick = onAction)
                        .semantics { contentDescription = action },
                    contentAlignment = Alignment.Center,
                ) {
                    if (play)
                        Box(
                            Modifier.size(36.dp).background(colors.onBackground.copy(alpha = 0.12f), CircleShape),
                            contentAlignment = Alignment.Center,
                        ) {
                            MediaIcon(Glyph.Play, Modifier.size(18.dp), colors.onBackground)
                        }
                    else MediaIcon(glyph, Modifier.size(24.dp), colors.onBackground)
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
    // The next column peeks in with its cover and the start of its title, so it reads as more
    // to swipe to rather than a second cover on the same row.
    val columnWidth = (LocalConfiguration.current.screenWidthDp.dp * 0.82f).coerceAtMost(420.dp)
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
                // Press and hold opens the menu; a column of ⋮ beside the next column was clutter.
                showMenu = false,
                modifier = Modifier.width(columnWidth),
            )
        }
    }
}

/**
 * "Jump back in": up to six recent covers in a grid, the quickest way back to what you were
 * listening to.
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
        Modifier.fillMaxWidth().padding(top = 8.dp).testTag("recent-tiles").semantics { contentDescription = title },
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        // No heading: the first row of what you played explains itself.
        // Large covers only, in a row that runs off the edge, as YouTube Music's speed dial: the
        // art is the label (each cover's name is its accessible label).
        androidx.compose.foundation.lazy.LazyRow(
            Modifier.fillMaxWidth().bleedHorizontally(if (inset == 0.dp) 16.dp else 0.dp),
            contentPadding = PaddingValues(horizontal = if (inset == 0.dp) 16.dp else inset),
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            listItems(cards.take(12), key = { it.key }) { media ->
                Box(
                    // Two whole covers and a slice of a third on any phone width.
                    Modifier.size(heroCoverSize())
                        .clip(MediaTokens.medium)
                        .combinedClickable(onClickLabel = "Play", onClick = { onPlay(media) })
                        .semantics { contentDescription = media.displayTitle }
                        .testTag("recent-${media.key}")
                ) {
                    Artwork(media, Modifier.fillMaxSize(), thumbnails, shape = MediaTokens.medium)
                    // The name on the cover, over a soft shade, as YouTube Music names its tiles.
                    Box(
                        Modifier.fillMaxSize().background(
                            androidx.compose.ui.graphics.Brush.verticalGradient(
                                0.45f to Color.Transparent,
                                1f to Color.Black.copy(alpha = 0.6f),
                            )
                        )
                    )
                    Text(
                        media.displayTitle,
                        Modifier.align(Alignment.BottomStart).padding(10.dp).clearAndSetSemantics {},
                        style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.Bold),
                        color = Color.White,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
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
                    // 256 dp, smaller on narrow phones so the first songs stay in view.
                    // 72% of the width, at most 256 dp and 30% of the height, so the first songs stay
                    // in view on short phones. (A width cap after fillMaxWidth had no effect.)
                    music ->
                        Modifier.size(
                            minOf(
                                LocalConfiguration.current.screenWidthDp.dp * 0.72f,
                                256.dp,
                                LocalConfiguration.current.screenHeightDp.dp * 0.3f,
                            )
                        )
                            .clip(MediaTokens.medium)
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
        // Centred under the centred cover, as YouTube Music's playlist pages.
        Column(
            Modifier.fillMaxWidth().padding(horizontal = textStart),
            horizontalAlignment = Alignment.CenterHorizontally,
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
                textAlign = androidx.compose.ui.text.style.TextAlign.Center,
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
            // One line under the title: who made it and how long it runs (else how many).
            // "Nova Lane, Mira Sol and more": the "and more" adds nothing next to the song list.
            val creator =
                cleanByline(opened?.creator.orEmpty()).takeUnless { person }.orEmpty()
                    .removeSuffix(" and more").removeSuffix(" & more")
            val length =
                if (feed.continuation == null && playable.isNotEmpty() && playable.all { it.value.optDouble("duration", 0.0) > 0 })
                    totalLength(playable.sumOf { it.value.optDouble("duration", 0.0) }.toLong())
                else ""
            val line =
                listOf(creator, length.ifBlank { count })
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
            if (opened != null && person) FollowButton(vm, id, opened.title, opened.channelThumbnail.ifBlank { opened.value.optString("thumbnail") })
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
                    // White with a dark glyph, the same as the player's: pink marks state only.
                    container = MaterialTheme.colorScheme.onBackground,
                    tint = MaterialTheme.colorScheme.background,
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
    /** Plays the songs found with this artist, from the round button at the card's end. */
    onPlay: (() -> Unit)? = null,
) {
    val person = media.kind == "artist" || media.kind == "channel"
    val shape = if (person) CircleShape else MediaTokens.medium
    // The card takes on its picture's colour over the page's wash, as YouTube Music's top result.
    val base = MaterialTheme.colorScheme.onBackground.copy(alpha = 0.08f)
    val tint = rememberArtworkTint(media, base, thumbnails)
    Row(
        Modifier.fillMaxWidth()
            .padding(horizontal = inset)
            .clip(MediaTokens.card)
            .background(base)
            .background(tint.copy(alpha = 0.22f))
            .combinedClickable(onClickLabel = "Open", onLongClickLabel = "More options", onLongClick = onMenu, onClick = onOpen)
            .padding(12.dp)
            .testTag("top-result"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        androidx.compose.runtime.CompositionLocalProvider(LocalArtworkPixels provides 0) {
            Artwork(media, Modifier.size(72.dp).clip(shape), thumbnails, shape = shape)
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(media.title, style = MaterialTheme.typography.titleLarge, maxLines = 2, overflow = TextOverflow.Ellipsis)
            // A round picture already says "artist".
            if (!person) Text(
                cleanByline(media.creator, keepKind = true).ifBlank { kindLabel(media.kind) },
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        // An icon, not words, as YouTube Music's top-result play.
        if (onPlay != null)
            RoundIconButton(
                Glyph.Play,
                "Play songs by ${media.title}",
                Modifier.testTag("top-result-play"),
                size = 48.dp,
                iconSize = 22.dp,
                container = MaterialTheme.colorScheme.onBackground,
                tint = MaterialTheme.colorScheme.background,
                action = onPlay,
            )
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
