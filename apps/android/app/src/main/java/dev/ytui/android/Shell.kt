package dev.ytui.android

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlin.math.abs
import kotlinx.coroutines.launch

/** Destinations in display order: (tab index, label, glyph). Tab 0 is the Library. */
private val DESTINATIONS =
    listOf(
        Triple(1, "Home", Glyph.Home),
        Triple(2, "Music", Glyph.Music),
        Triple(3, "Radio", Glyph.Radio),
        Triple(4, "Movies", Glyph.Film),
        Triple(0, "Library", Glyph.Library),
    )

/** Movies is an alpha test page; it is listed only when Settings turns it on. */
internal fun destinations(showMovies: Boolean) =
    if (showMovies) DESTINATIONS else DESTINATIONS.filter { it.first != 4 }

/** Name of a destination, as the dock and page title show it. */
internal fun destinationTitle(tab: Int): String =
    DESTINATIONS.firstOrNull { it.first == tab }?.second ?: "Home"

/**
 * Height of the floating mini player and navigation dock over the bottom of the page. Scrolling
 * content pads its end by this much, so it passes visibly under the dock but can still scroll
 * clear of it.
 */
internal val LocalBottomOverlay = compositionLocalOf { 0.dp }

/** The destination beside [tab] in display order ([forward] = +1 or -1), or null at an end. */
internal fun neighbourTab(tab: Int, forward: Int, showMovies: Boolean = false): Int? {
    val list = destinations(showMovies)
    val at = list.indexOfFirst { it.first == tab }
    if (at < 0) return null
    return list.getOrNull(at + forward)?.first
}

/** Page name in the display face; a new destination rises in as the old one sinks away. */
@Composable
internal fun PageTitle(title: String, modifier: Modifier = Modifier) {
    val motion = LocalMotion.current
    AnimatedContent(
        title,
        modifier,
        transitionSpec = {
            if (!motion.animate)
                androidx.compose.animation.EnterTransition.None togetherWith
                    androidx.compose.animation.ExitTransition.None
            else
                (fadeIn(motion.fade(220)) + slideInVertically(motion.enter(320)) { it / 3 }) togetherWith
                    (fadeOut(motion.fade(120)) + slideOutVertically(motion.exit(200)) { -it / 4 })
        },
        contentAlignment = Alignment.CenterStart,
        label = "page-title",
    ) { name ->
        Text(
            name,
            Modifier.testTag("feed-heading"),
            style = MaterialTheme.typography.headlineMedium,
            color = MaterialTheme.colorScheme.onBackground,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/**
 * Round account button. Being signed in is not marked with a badge: a dot on this button read as
 * an unread notification. The accessible name still says which state applies.
 */
@Composable
internal fun AccountButton(connected: Boolean, open: () -> Unit) {
    RoundIconButton(
        Glyph.Account,
        if (connected) "Your account" else "Sign in",
        Modifier.testTag("account-button"),
        action = open,
    )
}

/**
 * The phone navigation bar (tablets use [TabletSidebar]): each destination has its icon and a short label, as every major
 * media app does. The selected one has Material's indicator pill behind its icon.
 */
@Composable
internal fun AppNavigation(tab: Int, choose: (Int) -> Unit, showMovies: Boolean = false) {
    val motion = LocalMotion.current
    val colors = MaterialTheme.colorScheme
    val shown = destinations(showMovies)
    val highlight = colors.onBackground.copy(alpha = if (colors.background.luminance() < 0.4f) 0.11f else 0.08f)

    @Composable
    fun Cell(index: Int, title: String, glyph: Glyph, modifier: Modifier) {
        val selected = tab == index
        val source = remember(index) { MutableInteractionSource() }
        val tint by
            animateColorAsState(
                if (selected) colors.onBackground else colors.onSurfaceVariant,
                motion.fade(200),
                label = "nav-icon",
            )
        // Downloads catches covers flying in from Save, and bounces when one lands.
        val bounce = remember { Animatable(1f) }
        // Once this tab leaves the screen (the keyboard hides the phone dock), covers must not
        // aim at its stale position; they land on the temporary shortcut instead.
        if (index == 0) DisposableEffect(Unit) { onDispose { DownloadFlight.target = null } }
        if (index == 0)
            LaunchedEffect(DownloadFlight.landed) {
                if (DownloadFlight.landed > 0 && motion.animate) {
                    bounce.snapTo(1.25f)
                    bounce.animateTo(1f, spring(0.4f, Spring.StiffnessMediumLow))
                }
            }
        Column(
            modifier
                .selectable(selected, source, indication = null, role = Role.Tab) { choose(index) }
                .semantics { contentDescription = title }
                .testTag("tab-$index"),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(4.dp, Alignment.CenterVertically),
        ) {
            // Material's indicator: a 56×32dp pill centred behind the icon, opening sideways.
            val indicator by animateDpAsState(if (selected) 56.dp else 24.dp, motion.spring(), label = "nav-indicator")
            Box(Modifier.size(56.dp, 32.dp), contentAlignment = Alignment.Center) {
                if (selected)
                    Box(Modifier.size(indicator, 32.dp).clip(MediaTokens.pill).background(highlight))
                MediaIcon(
                    glyph,
                    Modifier.size(24.dp)
                        .then(if (index == 0) Modifier.trackCentre { DownloadFlight.target = it } else Modifier)
                        .pressScale(source, 0.9f)
                        .graphicsLayer {
                            scaleX = bounce.value
                            scaleY = bounce.value
                        },
                    tint,
                    filled = selected,
                )
            }
            Text(
                title,
                // The tab itself carries the name; the visible label is not read twice.
                Modifier.clearAndSetSemantics {},
                style = MaterialTheme.typography.labelSmall,
                color = tint,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }

    // Phones: a full-width bar in the page colour, as in the major media apps. The page
    // fades into it from above, and the mini player floats over that fade.
    Box(Modifier.fillMaxWidth().background(colors.background).padding(horizontal = 8.dp)) {
        Box(
            Modifier.fillMaxWidth()
                .height(64.dp)
                .selectableGroup()
                .testTag("bottom-navigation")
        ) {
            Row(Modifier.fillMaxSize()) {
                shown.forEach { (index, title, glyph) ->
                    Cell(index, title, glyph, Modifier.weight(1f).fillMaxHeight())
                }
            }
        }
    }
}


/**
 * Floating mini player above the dock. Its card takes on the artwork's colour, so what is playing
 * is recognisable at a glance; the progress line runs along its lower edge.
 */
@Composable
internal fun PlayerDock(vm: YtuiViewModel) {
    val live by vm.player.collectAsState()
    // Keep drawing the last item while the card leaves, so Stop or the close button slide it
    // away instead of it vanishing between frames.
    // A plain holder, not state: this function recomposes whenever `live` changes anyway.
    val last = remember { arrayOf(live) }
    if (live.media != null) last[0] = live
    val motion = LocalMotion.current
    androidx.compose.animation.AnimatedVisibility(
        live.media != null,
        // The card runs its own rise (see `appear`).
        enter = androidx.compose.animation.EnterTransition.None,
        exit =
            if (motion.animate)
                androidx.compose.animation.fadeOut(motion.fade(200)) +
                    androidx.compose.animation.slideOutVertically(motion.exit(260)) { it / 2 } +
                    androidx.compose.animation.shrinkVertically(motion.exit(300))
            else androidx.compose.animation.ExitTransition.None,
    ) {
        PlayerDockCard(vm, last[0])
    }
}

@Composable
private fun PlayerDockCard(vm: YtuiViewModel, player: PlayerState) {
    val config by vm.config.collectAsState()
    val mobile by vm.mobile.collectAsState()
    val media = player.media?.let(::MediaCard) ?: return
    val motion = LocalMotion.current
    val density = LocalDensity.current
    val colors = MaterialTheme.colorScheme
    val width = with(density) { LocalConfiguration.current.screenWidthDp.dp.toPx() }
    val gestures = mobile.gesture("gesture_mini_player")
    // Swipe sideways: next/previous; swipe up: expand; swipe down: stop and dismiss.
    val sideways = remember { Animatable(0f) }
    val vertical = remember { Animatable(0f) }
    val appear = remember { Animatable(if (motion.animate) 0f else 1f) }
    LaunchedEffect(Unit) { appear.animateTo(1f, motion.spring()) }
    // Bridge the gap between position snapshots so the bar glides instead of ticking;
    // seeks and track changes jump directly.
    val target = mediaProgress(player.position, player.duration)
    val glide = remember { Animatable(target) }
    LaunchedEffect(target) {
        if (!motion.animate || target < glide.value || target - glide.value > 0.05f)
            glide.snapTo(target)
        else glide.animateTo(target, tween(900, easing = LinearEasing))
    }
    // First mini player: show once that it can be swiped (unless gestures are off).
    val context = androidx.compose.ui.platform.LocalContext.current
    var hint by remember { mutableStateOf(false) }
    LaunchedEffect(gestures) {
        if (!gestures || MediaBridge.offlineUi || !GestureTips.pending(context, "mini")) return@LaunchedEffect
        kotlinx.coroutines.delay(1200)
        hint = true
        GestureTips.done(context, "mini")
        if (motion.animate) {
            val nudge = with(density) { 28.dp.toPx() }
            repeat(2) {
                sideways.animateTo(-nudge, motion.enter(260))
                sideways.animateTo(0f, motion.spring())
            }
        }
        kotlinx.coroutines.delay(3600)
        hint = false
    }
    val base = lerp(colors.background, colors.surfaceContainerHigh, 0.92f)
    val tint = rememberArtworkTint(media, base, config.optBoolean("thumbnails", true))
    androidx.compose.animation.AnimatedVisibility(
        hint,
        enter = androidx.compose.animation.fadeIn() + androidx.compose.animation.expandVertically(),
        exit = androidx.compose.animation.fadeOut() + androidx.compose.animation.shrinkVertically(),
    ) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 2.dp).testTag("mini-hint"),
            horizontalArrangement = Arrangement.Center,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                "Swipe sideways to skip · swipe down to stop",
                style = MaterialTheme.typography.labelMedium,
                color = colors.onSurfaceVariant,
            )
        }
    }
    val card = lerp(base, tint, 0.62f)
    val ink = mediaReadableText(colors.onSurface, card)
    val muted = mediaReadableText(colors.onSurfaceVariant, card)
    CompositionLocalProvider(LocalContentColor provides ink) {
        Box(
            Modifier.fillMaxWidth()
                // The page's 16dp gutter, so the card lines up with the content above it.
                .padding(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 4.dp)
                .graphicsLayer {
                    val lift = (1f - appear.value) * 28.dp.toPx()
                    translationX = sideways.value
                    translationY = vertical.value.coerceAtLeast(0f) + lift
                    val away =
                        maxOf(
                            abs(sideways.value) / width,
                            vertical.value.coerceAtLeast(0f) / (72.dp.toPx()),
                        )
                    alpha = appear.value.coerceIn(0f, 1f) * (1f - away.coerceIn(0f, 1f) * 0.6f)
                    val grow = 0.94f + 0.06f * appear.value.coerceIn(0f, 1f)
                    scaleX = grow
                    scaleY = grow
                }
                .swipeAxis(
                    enabled = gestures,
                    horizontal = true,
                    offset = sideways,
                    threshold = with(density) { 72.dp.toPx() },
                    onSwipe = { direction ->
                        val out = direction * width
                        sideways.animateTo(out, motion.exit(200))
                        vm.command(if (direction < 0) "next" else "previous")
                        sideways.snapTo(-out * 0.4f)
                        sideways.animateTo(0f, motion.spring())
                    },
                )
                .swipeAxis(
                    enabled = gestures,
                    horizontal = false,
                    offset = vertical,
                    threshold = with(density) { 40.dp.toPx() },
                    onSwipe = { direction ->
                        if (direction < 0) {
                            vm.playerOpen.value = true
                            vertical.snapTo(0f)
                        } else {
                            vertical.animateTo(with(density) { 96.dp.toPx() }, motion.exit(180))
                            // An easy gesture to make by accident: stopping offers Undo.
                            vm.stopWithUndo()
                        }
                    },
                )
                .shadow(14.dp, MediaTokens.card, ambientColor = Color.Black, spotColor = Color.Black)
                .clip(MediaTokens.card)
                .background(card)
                .clickable(onClickLabel = "Open player", role = Role.Button) {
                    vm.playerOpen.value = true
                }
                .testTag("mini-player")
        ) {
            Row(
                Modifier.fillMaxWidth().padding(start = 8.dp, end = 4.dp, top = 8.dp, bottom = 9.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Artwork(
                    media,
                    Modifier.size(46.dp).sharedCover(media.key, !LocalPlayerExpanded.current).clip(MediaTokens.medium),
                    config.optBoolean("thumbnails", true),
                    shape = MediaTokens.medium,
                )
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(1.dp)) {
                    Text(
                        if (player.video) player.title else MusicTitles.clean(player.title, media.creator),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        style = MaterialTheme.typography.titleSmall,
                        color = ink,
                    )
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        Text(
                            // A station between tracks is titled by its own name; say what it
                            // is rather than repeating that name.
                            (if (player.video) media.creator else cleanByline(media.creator))
                                .takeUnless { it.trim().equals(player.title.trim(), true) }.orEmpty().ifBlank {
                                if (player.preview) "Muted preview"
                                else if (media.live && media.kind == "station") "Live radio"
                                else if (media.live) "Live stream"
                                else "Now playing"
                            },
                            Modifier.weight(1f, fill = false),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            style = MaterialTheme.typography.bodySmall,
                            color = muted,
                        )
                        SleepBadge(player)
                    }
                }
                val source = remember { MutableInteractionSource() }
                Box(
                    Modifier.size(48.dp)
                        .pressScale(source, 0.88f)
                        .clip(CircleShape)
                        .clickable(source, SoftIndication, role = Role.Button) { vm.command("pause") }
                        .testTag("dock-pause")
                        .semantics { contentDescription = if (player.playing) "Pause" else "Play" },
                    contentAlignment = Alignment.Center,
                ) {
                    // A plain icon, like Next beside it: the mini player should not outshine the page.
                    PlayPauseIcon(player.playing, Modifier.size(26.dp), ink)
                }
                // The same control for music and video; closing is the swipe down.
                MediaIconButton(Glyph.Next, if (player.video) "Next video" else "Next track", "dock-next") {
                    vm.command("next")
                }
            }
            // On the flat part of the bottom edge: inset by the corner radius.
            if (player.duration > 0)
                Canvas(
                    // Inside the card: in from its sides and up from its lower edge.
                    Modifier.align(Alignment.BottomCenter)
                        .fillMaxWidth()
                        .padding(start = 12.dp, end = 12.dp, bottom = 5.dp)
                        .height(2.dp)
                ) {
                    drawLine(
                        ink.copy(alpha = 0.16f),
                        Offset(0f, size.height / 2),
                        Offset(size.width, size.height / 2),
                        size.height,
                    )
                    drawLine(
                        ink,
                        Offset(0f, size.height / 2),
                        Offset(size.width * glide.value, size.height / 2),
                        size.height,
                        StrokeCap.Round,
                    )
                }
        }
    }
}

/**
 * Tablet side panel: the playing item's colour washes down from the top of the card. Drag its
 * inner edge to resize it, or fold it into a slim strip with cover and play/pause.
 */
@Composable
internal fun TabletPlayerPanel(
    vm: YtuiViewModel,
    colors: androidx.compose.material3.ColorScheme,
    content: @Composable ColumnScope.() -> Unit,
) {
    val player by vm.player.collectAsState()
    val config by vm.config.collectAsState()
    val motion = LocalMotion.current
    val density = LocalDensity.current
    var collapsed by androidx.compose.runtime.saveable.rememberSaveable { mutableStateOf(false) }
    var width by androidx.compose.runtime.saveable.rememberSaveable { mutableFloatStateOf(352f) }
    val shown by
        androidx.compose.animation.core.animateDpAsState(
            if (collapsed) 96.dp else width.dp,
            motion.spring(),
            label = "panel-width",
        )
    val base = colors.surfaceContainerLow
    val media = player.media?.let(::MediaCard)
    val tint = rememberArtworkTint(media, base, config.optBoolean("thumbnails", true))
    Row(Modifier.fillMaxHeight().testTag("tablet-player-panel")) {
        // Resize grip between the content and the panel.
        Box(
            Modifier.width(16.dp)
                .fillMaxHeight()
                .padding(bottom = 20.dp)
                .then(
                    if (collapsed) Modifier
                    else
                        Modifier.pointerInput(Unit) {
                            detectHorizontalDragGestures { change, amount ->
                                change.consume()
                                width = (width - amount / density.density).coerceIn(300f, 480f)
                            }
                        }
                )
                .semantics { contentDescription = "Resize player panel" },
            contentAlignment = Alignment.Center,
        ) {
            if (!collapsed)
                Box(
                    Modifier.size(width = 4.dp, height = 40.dp)
                        .background(colors.onSurfaceVariant.copy(alpha = 0.35f), MediaTokens.pill)
                )
        }
        Box(
            Modifier.width(shown)
                .fillMaxHeight()
                .padding(end = 20.dp, bottom = 20.dp)
                .clip(MediaTokens.dock)
                .ambientGlow(tint, base, 0.7f)
        ) {
            if (collapsed)
                Column(
                    Modifier.fillMaxSize().padding(vertical = 14.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    RoundIconButton(Glyph.Back, "Expand player panel", Modifier.testTag("panel-expand")) {
                        collapsed = false
                    }
                    if (media != null)
                        Artwork(
                            media,
                            Modifier.size(56.dp).clip(MediaTokens.medium),
                            config.optBoolean("thumbnails", true),
                            shape = MediaTokens.medium,
                        )
                    PlayDisc(player.playing, size = 52.dp) { vm.command("pause") }
                    RoundIconButton(Glyph.Next, "Next track") { vm.command("next") }
                }
            else
                Column(
                    Modifier.verticalScroll(rememberScrollState()).padding(18.dp),
                    verticalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            "Now playing",
                            Modifier.weight(1f),
                            style = MaterialTheme.typography.titleLarge,
                            color = colors.onSurface,
                        )
                        RoundIconButton(
                            Glyph.Forward,
                            "Fold player panel",
                            Modifier.testTag("panel-collapse"),
                        ) {
                            collapsed = true
                        }
                    }
                    content()
                }
        }
    }
}

/** One-time gesture tips, remembered on this device; Settings can show them again. */
internal object GestureTips {
    private fun prefs(context: android.content.Context) =
        context.getSharedPreferences("ytui-tips", android.content.Context.MODE_PRIVATE)

    fun pending(context: android.content.Context, name: String) = !prefs(context).getBoolean(name, false)

    fun done(context: android.content.Context, name: String) = prefs(context).edit().putBoolean(name, true).apply()

    fun reset(context: android.content.Context) = prefs(context).edit().clear().apply()
}

@OptIn(androidx.compose.animation.ExperimentalSharedTransitionApi::class)
internal val LocalSharedScope =
    staticCompositionLocalOf<androidx.compose.animation.SharedTransitionScope?> { null }

/** True while the music player covers the app (the cover then belongs to the full player). */
internal val LocalPlayerExpanded = compositionLocalOf { false }

/**
 * The playing cover is one object: it flies between the mini player and the full player instead
 * of one fading out while another fades in. Each side says when it owns the cover.
 */
@OptIn(androidx.compose.animation.ExperimentalSharedTransitionApi::class)
@Composable
internal fun Modifier.sharedCover(key: String, visible: Boolean): Modifier {
    val shared = LocalSharedScope.current ?: return this
    if (!LocalMotion.current.animate) return this
    return with(shared) {
        // One shared cover for whatever is playing. A per-track key made every Next create a new
        // pair between the hidden mini player and the open player, which flew or mis-placed the
        // new cover instead of simply showing it.
        this@sharedCover.sharedElementWithCallerManagedVisibility(
            rememberSharedContentState("cover:now-playing"),
            visible,
        )
    }
}
