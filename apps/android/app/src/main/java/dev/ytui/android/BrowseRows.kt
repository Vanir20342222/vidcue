package dev.ytui.android

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
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
) {
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 60.dp)
            .clip(MediaTokens.medium)
            .combinedClickable(
                onClickLabel = "Play",
                onLongClickLabel = "More options",
                onLongClick = onMenu,
                onClick = onPlay,
            )
            .padding(start = 4.dp, top = 4.dp, bottom = 4.dp)
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
                        if (media.kind == "song" || media.kind == "video") "" else kindLabel(media.kind),
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
        durationOf(media).takeIf { it.isNotBlank() }?.let {
            Text(
                it,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        MediaIconButton(Glyph.More, "More options for ${media.title}", "media-menu-${media.key}", onMenu)
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
                music && (person || square) -> 56.dp
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
            val kind = kindLabel(media.kind).takeIf { media.collection || media.audio }.orEmpty()
            val byline =
                cleanByline(creator, keepKind = true).split(" · ").filter { it.isNotEmpty() && !it.equals(kind, true) }
            val details = (listOf(kind) + byline).filter { it.isNotBlank() }.joinToString(" · ")
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
        MediaIconButton(Glyph.More, "More options for ${media.title}", "media-menu-${media.key}", onMenu)
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
    Row(
        modifier.fillMaxWidth().padding(start = inset, end = if (inset > 0.dp) 8.dp else 0.dp, top = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            title,
            Modifier.weight(1f).semantics { heading() },
            style = MaterialTheme.typography.titleLarge,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        if (action != null)
            Pill(
                action,
                if (actionTag.isNotEmpty()) Modifier.testTag(actionTag) else Modifier,
                PillStyle.Quiet,
                glyph = actionGlyph,
                onClick = onAction,
            )
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
    val columnWidth = (LocalConfiguration.current.screenWidthDp.dp - inset - 72.dp).coerceAtMost(420.dp)
    LazyHorizontalGrid(
        GridCells.Fixed(rows),
        modifier.fillMaxWidth().height(64.dp * rows + 4.dp * (rows - 1)),
        contentPadding = PaddingValues(horizontal = (inset - 4.dp).coerceAtLeast(0.dp)),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
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
    // Text and buttons line up with the covers in the rows below.
    val textStart = if (inset == 0.dp) 20.dp else MediaTokens.textInset
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
            .ambientGlow(tint, background, 0.8f, softTop = true)
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
                    music -> Modifier.size(200.dp).clip(MediaTokens.medium)
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
            val line =
                listOf(kindLabel(kind), opened?.creator.orEmpty().takeUnless { person }.orEmpty(), count)
                    .filter { it.isNotBlank() }
                    // A non-breaking space keeps each "·" with the word before it.
                    .joinToString("\u00A0· ")
            if (line.isNotBlank())
                Text(
                    line,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 2,
                )
        }
        Row(
            Modifier.fillMaxWidth().padding(horizontal = textStart),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (opened != null && !person && !opened.live)
                RoundIconButton(
                    Glyph.Download,
                    "Download ${opened.title}",
                    Modifier.testTag("collection-download"),
                    container = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.08f),
                ) {
                    DownloadFlight.launch(opened, null)
                    vm.play(opened, download = true)
                }
            if (opened != null)
                RoundIconButton(
                    Glyph.Save,
                    "Save ${opened.title}",
                    Modifier.testTag("collection-save"),
                    container = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.08f),
                ) {
                    vm.save(opened)
                }
            Spacer(Modifier.weight(1f))
            if (playable.size > 1)
                RoundIconButton(
                    Glyph.Shuffle,
                    "Shuffle play",
                    Modifier.testTag("collection-shuffle"),
                    container = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.08f),
                ) {
                    vm.playAll(playable.shuffled())
                }
            if (playable.isNotEmpty())
                Pill(
                    "Play",
                    Modifier.testTag("collection-play"),
                    PillStyle.Primary,
                    glyph = Glyph.Play,
                ) {
                    vm.playAll(playable)
                }
        }
    }
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
        item {
            SuggestionRow(Glyph.Search, "Search for “$needle”", Modifier.testTag("suggestion-submit")) { onPick(needle) }
        }
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
