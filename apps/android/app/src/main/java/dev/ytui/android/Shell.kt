package dev.ytui.android

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.Animatable
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

/** Destinations in display order: (tab index, label, glyph). */
private val DESTINATIONS =
    listOf(
        Triple(1, "Home", Glyph.Home),
        Triple(2, "Music", Glyph.Music),
        Triple(3, "Radio", Glyph.Radio),
        Triple(0, "Downloads", Glyph.Download),
    )

/**
 * Height of the floating mini player and navigation dock over the bottom of the page. Scrolling
 * content pads its end by this much, so it passes visibly under the dock but can still scroll
 * clear of it.
 */
internal val LocalBottomOverlay = compositionLocalOf { 0.dp }

/** The destination beside [tab] in display order ([forward] = +1 or -1), or null at an end. */
internal fun neighbourTab(tab: Int, forward: Int): Int? {
    val at = DESTINATIONS.indexOfFirst { it.first == tab }
    if (at < 0) return null
    return DESTINATIONS.getOrNull(at + forward)?.first
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
 * The floating navigation dock. A soft highlight slides between destinations with a stretch: the
 * leading edge arrives first and the trailing edge follows, so the move reads as one gesture.
 */
@Composable
internal fun AppNavigation(tab: Int, choose: (Int) -> Unit, vertical: Boolean = false) {
    val density = LocalDensity.current
    val motion = LocalMotion.current
    val colors = MaterialTheme.colorScheme
    // Text merges unspecified fields (for example letter spacing) with the
    // ambient style. Measure that same resolved style at the user's font scale.
    val labelStyle = LocalTextStyle.current.merge(MaterialTheme.typography.labelMedium)
    val measurer = rememberTextMeasurer()
    val labelMeasurements =
        remember(measurer, labelStyle, density) {
            DESTINATIONS.associate { (index, title, _) ->
                index to
                    measurer
                        .measure(
                            AnnotatedString(title),
                            style = labelStyle.copy(fontWeight = FontWeight.Bold),
                            softWrap = false,
                        )
                        .size
            }
        }
    val widest = with(density) { labelMeasurements.values.maxOf { it.width }.toDp() + 2.dp }
    val labelHeight = with(density) { labelMeasurements.values.maxOf { it.height }.toDp() + 2.dp }
    val position = DESTINATIONS.indexOfFirst { it.first == tab }.coerceAtLeast(0)
    // Edges of the highlight, in cells; animated separately for the stretch.
    val lead = remember { Animatable(position.toFloat()) }
    val trail = remember { Animatable(position.toFloat()) }
    LaunchedEffect(position, motion) {
        val target = position.toFloat()
        if (!motion.animate) {
            lead.snapTo(target)
            trail.snapTo(target)
            return@LaunchedEffect
        }
        val forward = target > (lead.value + trail.value) / 2
        val quick = spring<Float>(0.82f, Spring.StiffnessMedium * 1.6f)
        val slow = spring<Float>(0.9f, Spring.StiffnessMediumLow * 1.4f)
        kotlinx.coroutines.coroutineScope {
            launch { (if (forward) lead else trail).animateTo(target, quick) }
            launch { (if (forward) trail else lead).animateTo(target, slow) }
        }
    }
    val dockColor = lerp(colors.background, colors.surfaceContainerHigh, 0.92f)
    val highlight = colors.onBackground.copy(alpha = if (colors.background.luminance() < 0.4f) 0.11f else 0.08f)

    @Composable
    fun Label(index: Int, title: String, alignStart: Boolean = false) {
        val selected = tab == index
        val tint by
            animateColorAsState(
                if (selected) colors.onBackground else colors.onSurfaceVariant,
                motion.fade(200),
                label = "nav-label",
            )
        Text(
            title,
            style = labelStyle,
            maxLines = 1,
            softWrap = false,
            textAlign = if (alignStart) TextAlign.Start else TextAlign.Center,
            fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium,
            color = tint,
            modifier =
                with(density) { labelMeasurements.getValue(index).width.toDp() + 2.dp }
                    .let { measured ->
                        // Beside an icon the label takes exactly its measured (bold) width, so
                        // its layout is as wide as the paragraph laid out in it and the group
                        // can stay centred. Under an icon it fills the cell, as before.
                        if (alignStart) Modifier.width(measured)
                        else Modifier.widthIn(min = measured).fillMaxWidth()
                    }
                    .testTag("navigation-label-$index"),
        )
    }

    @Composable
    fun Cell(index: Int, title: String, glyph: Glyph, modifier: Modifier, sideways: Boolean) {
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
        Box(
            modifier
                .selectable(selected, source, indication = null, role = Role.Tab) { choose(index) }
                .then(if (index == 0) Modifier.trackCentre { DownloadFlight.target = it } else Modifier)
                .testTag("tab-$index"),
            contentAlignment = Alignment.Center,
        ) {
            Box(
                Modifier.pressScale(source, 0.9f).graphicsLayer {
                    scaleX = bounce.value
                    scaleY = bounce.value
                },
                contentAlignment = Alignment.Center,
            ) {
                if (sideways)
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        MediaIcon(glyph, tint = tint, filled = selected)
                        Label(index, title, alignStart = true)
                    }
                else
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(3.dp),
                    ) {
                        MediaIcon(glyph, tint = tint, filled = selected)
                        Label(index, title)
                    }
            }
        }
    }

    val dock =
        Modifier.shadow(16.dp, MediaTokens.dock, ambientColor = Color.Black, spotColor = Color.Black)
            .clip(MediaTokens.dock)
            .background(dockColor)
            .border(1.dp, colors.outlineVariant, MediaTokens.dock)

    if (vertical)
        BoxWithConstraints(Modifier.fillMaxHeight().padding(start = 12.dp, top = 4.dp, bottom = 16.dp)) {
            // Short windows (phone landscape) must still show all four labelled
            // destinations: share the height evenly before falling back to scrolling.
            val natural = maxOf(76.dp, labelHeight + 48.dp)
            val fit = (maxHeight - 16.dp) / DESTINATIONS.size
            val compact = fit < natural + 8.dp
            val itemHeight = if (compact) maxOf(fit, labelHeight + 40.dp) else natural
            val width = maxOf(84.dp, widest + 24.dp)
            Box(
                Modifier.width(width)
                    .then(dock)
                    .verticalScroll(rememberScrollState())
                    .selectableGroup()
                    .testTag("navigation-rail")
                    .padding(vertical = 8.dp)
            ) {
                Highlight(lead.value, trail.value, itemHeight, highlight, vertical = true)
                Column(Modifier.fillMaxWidth()) {
                    DESTINATIONS.forEach { (index, title, glyph) ->
                        Cell(index, title, glyph, Modifier.fillMaxWidth().height(itemHeight), false)
                    }
                }
            }
        }
    else
        BoxWithConstraints(
            Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp, top = 4.dp, bottom = 10.dp)
        ) {
            // Adapt to measured full-size words, not a font-scale guess or a font cap.
            val twoRows = maxWidth < (widest + 24.dp) * 4
            val sidewaysCells = maxWidth / 2 >= widest + 52.dp
            if (twoRows) {
                Column(Modifier.fillMaxWidth().then(dock).selectableGroup().testTag("bottom-navigation")) {
                    DESTINATIONS.chunked(2).forEach { row ->
                        Row(Modifier.fillMaxWidth()) {
                            row.forEach { (index, title, glyph) ->
                                Cell(
                                    index,
                                    title,
                                    glyph,
                                    Modifier.weight(1f)
                                        .heightIn(
                                            min =
                                                // The cell's own padding (4dp outside the highlight,
                                                // 4dp inside) is 16dp of this, and the tab itself
                                                // must still reach the 48dp touch target.
                                                if (sidewaysCells) maxOf(66.dp, labelHeight + 24.dp)
                                                else maxOf(76.dp, labelHeight + 44.dp)
                                        )
                                        .padding(4.dp)
                                        .clip(MediaTokens.medium)
                                        .background(if (tab == index) highlight else Color.Transparent)
                                        .padding(horizontal = 4.dp, vertical = 4.dp),
                                    sidewaysCells,
                                )
                            }
                        }
                    }
                }
            } else {
                val height = maxOf(64.dp, labelHeight + 42.dp)
                val cell = maxWidth / DESTINATIONS.size
                Box(
                    Modifier.fillMaxWidth()
                        .height(height)
                        .then(dock)
                        .selectableGroup()
                        .testTag("bottom-navigation")
                ) {
                    Highlight(lead.value, trail.value, cell, highlight, vertical = false)
                    Row(Modifier.fillMaxSize()) {
                        DESTINATIONS.forEach { (index, title, glyph) ->
                            Cell(index, title, glyph, Modifier.weight(1f).fillMaxHeight(), false)
                        }
                    }
                }
            }
        }
}

@Composable
private fun BoxScope.Highlight(lead: Float, trail: Float, cell: Dp, color: Color, vertical: Boolean) {
    val start = minOf(lead, trail)
    val span = abs(lead - trail) + 1f
    val inset = 6.dp
    Box(
        Modifier.then(
                if (vertical)
                    Modifier.offset(y = cell * start).height(cell * span).fillMaxWidth()
                else Modifier.offset(x = cell * start).width(cell * span).fillMaxHeight()
            )
            .padding(inset)
            .clip(androidx.compose.foundation.shape.RoundedCornerShape(18.dp))
            .background(color)
    )
}

/**
 * Floating mini player above the dock. Its card takes on the artwork's colour, so what is playing
 * is recognisable at a glance; the progress line runs along its lower edge.
 */
@Composable
internal fun PlayerDock(vm: YtuiViewModel) {
    val player by vm.player.collectAsState()
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
                "Swipe sideways to skip · swipe down to close",
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
                .padding(horizontal = 12.dp, vertical = 4.dp)
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
                            vm.command("stop")
                        }
                    },
                )
                .shadow(14.dp, MediaTokens.dock, ambientColor = Color.Black, spotColor = Color.Black)
                .clip(MediaTokens.dock)
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
                        player.title,
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
                            media.creator.ifBlank {
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
                    Box(
                        Modifier.size(40.dp).background(ink, CircleShape),
                        contentAlignment = Alignment.Center,
                    ) {
                        PlayPauseIcon(player.playing, Modifier.size(20.dp), card)
                    }
                }
                // The mini player can be dismissed; this is the existing stop command.
                MediaIconButton(Glyph.Close, "Stop and close player", "dock-close") {
                    vm.command("stop")
                }
            }
            if (player.duration > 0)
                Canvas(Modifier.align(Alignment.BottomCenter).fillMaxWidth().height(3.dp)) {
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
                        StrokeCap.Butt,
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
