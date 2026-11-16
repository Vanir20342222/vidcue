package dev.ytui.android

import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.SizeTransform
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.withInfiniteAnimationFrameNanos
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.scaleIn
import androidx.compose.animation.core.LinearEasing
import androidx.compose.foundation.*
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.*
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.zIndex
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.compose.ui.graphics.luminance
import androidx.core.view.WindowInsetsCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import kotlin.math.*
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import org.json.JSONObject

/** Android presentation only. Provider/state operations go through the Rust repository. */
object ComposeHost {
    internal val signals = MutableStateFlow(0L)
    internal val fullscreen = MutableStateFlow(false)

    @JvmStatic
    fun fullscreenChanged(value: Boolean) {
        fullscreen.value = value
    }

    /** Last applied app palette, so the sign-in activity matches the chosen theme. */
    internal var colorScheme: ColorScheme? = null

    /** Type that goes with [colorScheme]; Nothing palettes use dot-matrix display type. */
    internal var typography: androidx.compose.material3.Typography = MediaTokens.typography

    /**
     * Last applied motion settings, so the sign-in, fullscreen and Movies windows animate with the
     * same style and speed as the app instead of fixed durations.
     */
    @Volatile internal var motion: Motion = Motion()

    /** [motion] for a secondary window; [reduced] adds that window's own system-animation check. */
    internal fun motionFor(reduced: Boolean): Motion = motion.copy(reduced = motion.reduced || reduced)

    private val authWorking = MutableStateFlow(false)
    private val authStatus = MutableStateFlow("")

    @JvmStatic fun authBusy() = authWorking.value

    @JvmStatic
    fun setAuthBusy(value: Boolean) {
        authWorking.value = value
    }

    @JvmStatic
    fun authMessage(text: String) {
        authStatus.value = text
    }

    @JvmStatic
    fun resetAuth() {
        authWorking.value = false
        authStatus.value = ""
    }

    @OptIn(ExperimentalMaterial3Api::class)
    @JvmStatic
    fun attachAuth(activity: AuthActivity, web: android.webkit.WebView) {
        activity.setContent {
            val busy by authWorking.collectAsState()
            val message by authStatus.collectAsState()
            val dark = isSystemInDarkTheme()
            val scheme = colorScheme ?: if (dark) darkColorScheme() else lightColorScheme()
            // Same edge-to-edge treatment as the main window: the page colour runs under the
            // status and gesture bars instead of a separate system band.
            SideEffect {
                val window = activity.window
                WindowCompat.setDecorFitsSystemWindows(window, false)
                window.statusBarColor = android.graphics.Color.TRANSPARENT
                window.navigationBarColor = android.graphics.Color.TRANSPARENT
                if (android.os.Build.VERSION.SDK_INT >= 29) {
                    window.isNavigationBarContrastEnforced = false
                    window.isStatusBarContrastEnforced = false
                }
                val light = scheme.background.luminance() > 0.5f
                WindowCompat.getInsetsController(window, window.decorView).apply {
                    isAppearanceLightStatusBars = light
                    isAppearanceLightNavigationBars = light
                }
            }
            MaterialTheme(colorScheme = scheme, typography = typography) {
                CompositionLocalProvider(
                    LocalContentColor provides scheme.onBackground,
                    LocalIndication provides SoftIndication,
                    LocalRippleConfiguration provides null,
                    LocalMotion provides motionFor(AuthUi.reducedMotion),
                ) {
                    AuthScreen(activity, web, busy, message)
                }
            }
        }
    }

    @JvmStatic
    fun trimMemory() {
        ArtworkCache.trimMemory()
    }

    @JvmStatic
    fun changed() {
        signals.update { it + 1 }
    }

    @JvmStatic
    fun attach(activity: RustActivity) {
        WindowCompat.setDecorFitsSystemWindows(activity.window, false)
        activity.setContent { YtuiApp(activity) }
    }
}

internal fun JSONObject.color(key: String, fallback: Color, neutralPreset: Boolean = true): Color {
    if (neutralPreset && optString("name") == "vidcue-dark")
        when (key) {
            "background" -> return MediaTokens.neutralBackground
            "surface" -> return MediaTokens.neutralSurface
            "text" -> return MediaTokens.neutralText
        }
    val rgb = optJSONArray(key) ?: return fallback
    return Color(android.graphics.Color.rgb(rgb.optInt(0), rgb.optInt(1), rgb.optInt(2)))
}

internal fun time(seconds: Float): String {
    val value = seconds.takeIf { it.isFinite() }?.toLong()?.coerceAtLeast(0) ?: 0
    return if (value >= 3600) "%d:%02d:%02d".format(value / 3600, value / 60 % 60, value % 60)
    else "%d:%02d".format(value / 60, value % 60)
}

@OptIn(ExperimentalMaterial3Api::class, androidx.compose.animation.ExperimentalSharedTransitionApi::class)
@Composable
private fun YtuiApp(activity: RustActivity, vm: YtuiViewModel = viewModel()) {
    val dark = isSystemInDarkTheme()
    val palette by vm.palette.collectAsState()
    val config by vm.config.collectAsState()
    val mobile by vm.mobile.collectAsState()
    val tab by vm.tab.collectAsState()
    val account by vm.account.collectAsState()
    val playerOpen by vm.playerOpen.collectAsState()
    val fullscreen by ComposeHost.fullscreen.collectAsState()
    val videoPlayer by
        remember(vm) { vm.player.map { it.video }.distinctUntilChanged() }.collectAsState(false)
    val hasPlayer by
        remember(vm) { vm.player.map { it.media != null }.distinctUntilChanged() }
            .collectAsState(false)
    val ready by vm.ready.collectAsState()
    // Notifications matter once something plays (background controls) or downloads (progress),
    // so the permission is asked then, with a reason in view, not on the first look at Home.
    val transferring by
        remember(vm) { vm.downloads.map { list -> list.any { it.optString("state") != "Complete" } }.distinctUntilChanged() }
            .collectAsState(false)
    LaunchedEffect(hasPlayer, transferring) {
        if (hasPlayer || transferring) NotificationPermission.requestOnce(activity)
    }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    val keyboard = LocalSoftwareKeyboardController.current
    val focusManager = LocalFocusManager.current
    val imeVisible = WindowInsets.ime.getBottom(LocalDensity.current) > 0
    var searchFocused by remember(tab) { mutableStateOf(false) }
    // Leaving search moves input focus to this non-text sink. Clearing focus
    // instead lets Android hand focus back to the Compose root view, which then
    // re-enters the first focusable node (the search field) and reopens the IME.
    var scrollToTop by remember { mutableIntStateOf(0) }
    fun selectTab(index: Int) {
        if (index == tab) scrollToTop++
        vm.chooseTab(index)
    }
    val focusSink = remember { FocusRequester() }
    // Leaving search lands on the results grid (PR #31's destination); the sink
    // remains the fallback when no grid is composed (e.g. during a tab change).
    val browseFocus = remember(tab) { FocusRequester() }
    fun releaseSearchFocus() {
        if (
            runCatching { browseFocus.requestFocus() }.isFailure &&
                runCatching { focusSink.requestFocus() }.isFailure
        )
            focusManager.clearFocus(force = true)
        keyboard?.hide()
        searchFocused = false
    }
    // The player covers the page but leaves it composed, so a search field there
    // (browse or the offline library filter) would keep input focus underneath and
    // reopen the IME whenever the window regains focus. Opening the player moves
    // focus to the non-text sink; fields inside the player's own sheets are unaffected.
    LaunchedEffect(playerOpen) {
        if (playerOpen) {
            if (runCatching { focusSink.requestFocus() }.isFailure)
                focusManager.clearFocus(force = true)
            keyboard?.hide()
        }
    }
    val searchRequestState =
        remember(vm, tab) {
            val feed = vm.feeds[tab]
            // Filtered searches (Videos, Songs, ...) are search results too.
            val initial = SearchRequests.isSearch(feed.value.request)
            feed.map { SearchRequests.isSearch(it.request) }.distinctUntilChanged() to initial
        }
    val searchResults =
        key(tab) { searchRequestState.first.collectAsState(searchRequestState.second).value }
    val searchMode = searchFocused || searchResults
    val showMovies = mobile.optBoolean("show_movies", false)
    // Turning Movies off while it is open returns to Home.
    LaunchedEffect(showMovies, tab) { if (!showMovies && tab == 4) vm.chooseTab(1) }
    var dialog by rememberSaveable { mutableStateOf("") }
    // First-run setup. Instrumented offline launches opt in explicitly so existing
    // UI tests keep starting on the feed.
    var onboarding by rememberSaveable {
        mutableStateOf(
            OnboardingStore.shouldShow(activity) &&
                (!MediaBridge.offlineUi ||
                    activity.intent.getBooleanExtra("ytui.onboarding", false))
        )
    }
    // Imported documents can legitimately reuse a built-in name. Their paint
    // roles must remain exactly the imported values rather than the default preset.
    val neutralPreset = config.optString("theme", "system") != "custom"
    val background = palette.color("background", Color(0xff101418), neutralPreset)
    val surface = palette.color("surface", Color(0xff202830), neutralPreset)
    val foreground = palette.color("text", Color(0xffd5dee6), neutralPreset)
    // A palette without an accent stays neutral rather than borrowing a fixed hue.
    val accent = palette.color("accent", foreground, neutralPreset)
    val light = palette.optBoolean("light")
    val base = if (light) lightColorScheme() else darkColorScheme()
    val colors =
        base.copy(
            primary = accent,
            onPrimary = mediaOnAccent(accent),
            primaryContainer = lerp(background, accent, 0.16f),
            onPrimaryContainer = foreground,
            secondary = accent,
            secondaryContainer = lerp(surface, accent, 0.14f),
            onSecondaryContainer = foreground,
            background = background,
            onBackground = foreground,
            surface = surface,
            onSurface = foreground,
            onSurfaceVariant = mediaReadableText(lerp(background, foreground, 0.7f), surface),
            surfaceVariant = lerp(background, surface, 0.65f),
            surfaceContainer = lerp(background, surface, 0.72f),
            surfaceContainerLow = lerp(background, surface, 0.4f),
            surfaceContainerHigh = surface,
            outline = lerp(background, foreground, 0.35f),
            outlineVariant = lerp(background, foreground, 0.12f),
        )
    // Imported ("custom") palettes may reuse a built-in name; they keep the default type.
    val typography = if (neutralPreset) YtuiType.forPalette(palette) else MediaTokens.typography
    SideEffect {
        ComposeHost.colorScheme = colors
        ComposeHost.typography = typography
        AuthUi.motion(config.optBoolean("reduced_motion"))
    }
    LaunchedEffect(Unit) { vm.initialize(dark) }
    LaunchedEffect(dark) { vm.systemTheme(dark) }
    LaunchedEffect(lifecycle) {
        lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) {
            coroutineScope {
                launch { ComposeHost.signals.collect { vm.snapshot() } }
                launch {
                    while (isActive) {
                        delay(1000)
                        vm.snapshot()
                    }
                }
            }
        }
    }
    SideEffect {
        activity.window.statusBarColor = android.graphics.Color.TRANSPARENT
        // The app draws behind the gesture bar itself: the root paints the page colour and the
        // music player continues its cover-coloured backdrop there. An opaque bar colour (or the
        // system's contrast scrim) showed as a dark band under the player's action row.
        activity.window.navigationBarColor = android.graphics.Color.TRANSPARENT
        if (android.os.Build.VERSION.SDK_INT >= 29) activity.window.isNavigationBarContrastEnforced = false
        WindowCompat.getInsetsController(activity.window, activity.window.decorView).apply {
            isAppearanceLightStatusBars = light
            isAppearanceLightNavigationBars = light
        }
    }
    BackHandler {
        // Read platform visibility at dispatch time: the first Back dismisses
        // the IME, while the next leaves the search session without losing text.
        val keyboardShowing =
            ViewCompat.getRootWindowInsets(activity.window.decorView)
                ?.isVisible(WindowInsetsCompat.Type.ime()) == true
        if (keyboardShowing) keyboard?.hide()
        else if (dialog.isNotBlank()) dialog = ""
        else if (playerOpen) vm.playerOpen.value = false
        else if (searchMode) {
            releaseSearchFocus()
            if (searchResults) vm.back()
        } else if (!vm.back()) activity.moveTaskToBack(true)
    }
    val motion =
        remember(mobile, config.optBoolean("reduced_motion")) {
            Motion.from(mobile, config.optBoolean("reduced_motion"))
        }
    SideEffect { ComposeHost.motion = motion }
    // Once the music player has finished opening it covers the whole window, so the page
    // underneath stops drawing (it stays composed). Dragging the player down or closing it shows
    // the page again at once.
    var musicPlayerCovers by remember { mutableStateOf(false) }
    LaunchedEffect(playerOpen, videoPlayer, motion) {
        musicPlayerCovers = false
        if (playerOpen && !videoPlayer) {
            delay(motion.millis(400).toLong() + 100)
            musicPlayerCovers = true
        }
    }
    MaterialTheme(
        colorScheme = colors,
        typography = typography,
        shapes =
            Shapes(
                small = MediaTokens.small,
                medium = MediaTokens.medium,
                large = MediaTokens.dialog,
            ),
    ) {
        CompositionLocalProvider(
            LocalContentColor provides foreground,
            LocalReducedMotion provides config.optBoolean("reduced_motion"),
            LocalMotion provides motion,
            // App-wide press feedback is the soft wash, not Material's ripple.
            LocalIndication provides SoftIndication,
            LocalRippleConfiguration provides null,
        ) {
            androidx.compose.animation.SharedTransitionLayout(Modifier.fillMaxSize()) {
            CompositionLocalProvider(
                LocalSharedScope provides this,
                LocalPlayerExpanded provides (playerOpen && !videoPlayer),
            ) {
            Box(Modifier.fillMaxSize()) {
                // While setup is shown the app beneath is not composed at all, so neither
                // touch, keyboard focus, TalkBack nor a notification-opened player can reach
                // it. Feeds and settings keep loading in the view model.
                if (!onboarding) BoxWithConstraints(
                    Modifier.fillMaxSize()
                        .background(background)
                        // The video panel is opaque. Keep browse state/artwork
                        // attached, but skip drawing the covered content.
                        .graphicsLayer {
                            alpha =
                                if (playerOpen && (videoPlayer || (musicPlayerCovers && !PanelPull.active.value))) 0f
                                else 1f
                        }
                        .then(if (playerOpen) Modifier.clearAndSetSemantics {} else Modifier)
                        .testTag("app-background")
                        .semantics { stateDescription = if (ready) "Ready" else "Loading settings" }
                        .safeDrawingPadding()
                        .imePadding()
                ) {
                    val tabletLayout = maxWidth >= 840.dp
                    val compactSidebar = maxWidth < 1000.dp
                    Column(Modifier.fillMaxSize()) {
                        // First in traversal order, so a root re-entry also lands here.
                        Spacer(Modifier.size(0.dp).focusRequester(focusSink).focusable())
                        // The page title and actions; on tablets they sit over the content
                        // column, beside the navigation dock, instead of spanning the window.
                        @Composable
                        fun Toolbar() {
                        AnimatedVisibility(
                            visible = !searchMode,
                            enter =
                                if (!motion.animate) EnterTransition.None
                                else
                                    fadeIn(motion.fade(160)) +
                                        expandVertically(
                                            motion.enter(240),
                                            expandFrom = Alignment.Top,
                                        ),
                            exit =
                                if (!motion.animate) ExitTransition.None
                                else
                                    fadeOut(motion.fade(120)) +
                                        shrinkVertically(
                                            motion.exit(200),
                                            shrinkTowards = Alignment.Top,
                                        ),
                        ) {
                            AppToolbar(
                                vm,
                                tab,
                                connected = account.optBoolean("connected"),
                                onSearch = { searchFocused = true },
                                onYou = { dialog = "settings" },
                                // Tablets keep account and settings at the foot of the sidebar.
                                showYou = !tabletLayout,
                            )
                        }
                        }
                        if (!tabletLayout) Toolbar()
                        // Phones: the mini player and navigation float over the page instead of
                        // taking a band of their own, so artwork stays visible around them.
                        val bottomDock = !tabletLayout && !imeVisible
                        var dockHeight by remember { mutableIntStateOf(0) }
                        Box(Modifier.weight(1f).fillMaxWidth()) {
                            val tablet = tabletLayout
                            CompositionLocalProvider(
                                LocalBottomOverlay provides
                                    if (bottomDock) with(LocalDensity.current) { dockHeight.toDp() } else 0.dp
                            ) {
                            Row(Modifier.fillMaxSize()) {
                                if (tablet)
                                    TabletSidebar(
                                        tab,
                                        ::selectTab,
                                        connected = account.optBoolean("connected"),
                                        compact = compactSidebar,
                                        showMovies = showMovies,
                                    ) { name ->
                                        dialog = name
                                        if (name == "queue") vm.loadQueue()
                                        if (name == "history") vm.loadHistory()
                                    }
                                Column(Modifier.weight(1f).fillMaxHeight()) {
                                if (tablet) Toolbar()
                                Box(
                                    Modifier.weight(1f)
                                        .fillMaxWidth()
                                        .tabSwipe(tab, enabled = mobile.gesture("gesture_swipe_tabs") && !searchMode, showMovies = showMovies) {
                                            vm.chooseTab(it)
                                        }
                                ) {
                                    if (tab == 0)
                                        LibraryPage(vm, onAccount = { dialog = "account" }) { chips ->
                                            BrowsePane(
                                                vm,
                                                0,
                                                onAccount = { dialog = "account" },
                                                onFeeds = {},
                                                searchActive = false,
                                                onSearchFocus = {},
                                                releaseSearchFocus = ::releaseSearchFocus,
                                                browseFocus = browseFocus,
                                                scrollToTop = scrollToTop,
                                                library = true,
                                                chips = chips,
                                            )
                                        }
                                    else if (tab == 4) MoviesPage(vm)
                                    else
                                        key(tab) {
                                            BrowsePane(
                                                vm,
                                                tab,
                                                onAccount = { dialog = "account" },
                                                onFeeds = { dialog = "feeds" },
                                                searchActive = searchFocused,
                                                onSearchFocus = { searchFocused = it },
                                                releaseSearchFocus = ::releaseSearchFocus,
                                                browseFocus = browseFocus,
                                                scrollToTop = scrollToTop,
                                            )
                                        }
                                }
                                }
                                if (tablet && hasPlayer) {
                                    TabletPlayerPanel(vm, colors) {
                                        PlayerPane(
                                            vm,
                                            expanded = false,
                                            showSurface = !playerOpen && !fullscreen,
                                            onQueue = {
                                                dialog = "queue"
                                                vm.loadQueue()
                                            },
                                            onSleep = { dialog = "sleep" },
                                            onSettings = { dialog = "player-options" },
                                        )
                                    }
                                }
                            }
                            }
                            if (bottomDock)
                                Column(
                                    Modifier.align(Alignment.BottomCenter)
                                        .fillMaxWidth()
                                        .onSizeChanged { dockHeight = it.height }
                                        // The page fades out behind the player and navigation, so
                                        // nothing shows through between and beside them.
                                        .drawBehind {
                                            val page = colors.background
                                            drawRect(
                                                androidx.compose.ui.graphics.Brush.verticalGradient(
                                                    0f to page.copy(alpha = 0f),
                                                    0.28f to page.copy(alpha = 0.94f),
                                                    1f to page,
                                                )
                                            )
                                        }
                                        .testTag("bottom-dock")
                                ) {
                                    PlayerDock(vm)
                                    AppNavigation(tab, ::selectTab, showMovies = showMovies)
                                }
                        }
                    }
                }
                if (!fullscreen && !onboarding) {
                    @Composable
                    fun PlaybackPanel(allowVideoSurface: Boolean) {
                        // Music borrows its cover's colour for the whole screen.
                        val nowMedia by
                            remember(vm) {
                                    vm.player
                                        .map { it.media?.let(::MediaCard) }
                                        .distinctUntilChanged { a, b ->
                                            a?.key == b?.key && a?.artworkSource == b?.artworkSource
                                        }
                                }
                                .collectAsState(null)
                        val glow =
                            rememberArtworkTint(
                                nowMedia,
                                background,
                                !videoPlayer && config.optBoolean("thumbnails", true),
                            )
                        Panel(
                            "Now playing",
                            { vm.playerOpen.value = false },
                            full = true,
                            showHeader = !videoPlayer,
                            scroll = false,
                            swipeToClose = mobile.gesture("gesture_swipe_dismiss"),
                            enterFromBottom = true,
                            backdrop =
                                if (videoPlayer) Modifier else Modifier.ambientGlow(glow, background),
                        ) {
                            FocusedPlayer(
                                vm,
                                allowVideoSurface = allowVideoSurface,
                                minimize = { vm.playerOpen.value = false },
                                onQueue = {
                                    dialog = "queue"
                                    vm.loadQueue()
                                },
                                onMore = { dialog = "player-options" },
                                onLyrics = {
                                    dialog = "lyrics"
                                    vm.player.value.media?.let(vm::lyricLoad)
                                },
                                onSleep = { dialog = "sleep" },
                                onOutput = { dialog = "output" },
                            )
                        }
                    }
                    // Each opening gets a fresh panel. Reopened during the closing fade, the
                    // old one was reused still "leaving" and lowered, so Minimize did nothing
                    // and the app behind stayed hidden from accessibility.
                    var panelOpening by remember { mutableIntStateOf(0) }
                    LaunchedEffect(playerOpen) { if (playerOpen) panelOpening++ }
                    // Changing animation parameters mid-transition retains the old
                    // graphics layer. A native video view must never inherit it.
                    if (videoPlayer) {
                        if (playerOpen) PlaybackPanel(allowVideoSurface = true)
                    } else {
                        AnimatedVisibility(
                            visible = playerOpen,
                            // The panel itself rises from the bottom and flies back down
                            // (Panel enterFromBottom), mirroring the swipe-away. Only a close
                            // that bypasses the panel (Stop, a shortcut) fades it here.
                            enter = EnterTransition.None,
                            exit =
                                if (motion.animate) fadeOut(motion.exit(200))
                                else ExitTransition.None,
                        ) {
                            key(panelOpening) { PlaybackPanel(allowVideoSurface = false) }
                        }
                    }
                }
                if (!onboarding) when (dialog) {
                    "player-options" ->
                        Panel("Playback options", { dialog = "" }) {
                            PlayerPane(
                                vm,
                                expanded = true,
                                showSurface = false,
                                onQueue = {
                                    dialog = "queue"
                                    vm.loadQueue()
                                },
                                onSleep = { dialog = "sleep" },
                                onDone = { dialog = "" },
                                onOutput = { dialog = "output" },
                            )
                        }
                    "settings" ->
                        SettingsPanel(
                            vm,
                            dark,
                            onAccount = { dialog = "account" },
                            onOpen = { name ->
                                dialog = name
                                if (name == "queue") vm.loadQueue()
                                if (name == "history") vm.loadHistory()
                            },
                            onSetup = {
                                // Replay keeps the saved completion, so a process death
                                // mid-replay does not turn it into a first run again.
                                dialog = ""
                                onboarding = true
                            },
                        ) {
                            dialog = ""
                        }
                    "account" -> AccountPanel(vm) { dialog = "" }
                    "feeds" -> FeedPanel(vm) { dialog = "" }
                    "queue" -> QueuePanel(vm) { dialog = "" }
                    "history" -> HistoryPanel(vm) { dialog = "" }
                    "lyrics" ->
                        LyricsPanel(vm) {
                            vm.lyricClose()
                            dialog = ""
                        }
                    "sleep" ->
                        Panel("Sleep timer", { dialog = "" }) { SleepOptions(vm) { dialog = "" } }
                    "output" ->
                        Panel("Audio output", { dialog = "" }) { OutputOptions(vm) { dialog = "" } }
                    "about" ->
                        Panel("About Vidcue", { dialog = "" }) {
                            Text(
                                "Vidcue plays videos, music and radio, and keeps downloads for when " +
                                    "you are offline. This is a preview build.",
                                style = MaterialTheme.typography.bodyLarge,
                            )
                            Text(
                                "Version ${BuildConfig.VERSION_NAME} (${BuildConfig.VERSION_CODE}) · build ${BuildConfig.SOURCE_REVISION.take(7)}",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                            AppUpdateSettings()
                            ValueRow("Radio artwork credits", glyph = Glyph.Link) {
                                vm.effect(
                                    "open",
                                    "url" to
                                        "https://github.com/Vanir20342222/vidcue/blob/main/docs/android/RADIO_ARTWORK.md",
                                )
                            }
                            ValueRow("Help and documentation", glyph = Glyph.Link) {
                                vm.effect(
                                    "open",
                                    "url" to
                                        "https://github.com/Vanir20342222/vidcue/tree/main/docs/android",
                                )
                            }
                        }
                }
                if (!onboarding && !fullscreen)
                    DownloadFlightHost(
                        config.optBoolean("thumbnails", true),
                        // The full-screen player and panels cover the Downloads tab.
                        navigationHidden = playerOpen || dialog.isNotEmpty(),
                        downloads = vm.downloads,
                    ) {
                        dialog = ""
                        vm.playerOpen.value = false
                        selectTab(0)
                    }
                if (!onboarding && !fullscreen)
                    NoticeHost(
                        vm,
                        bottom =
                            with(LocalDensity.current) {
                                WindowInsets.safeDrawing.getBottom(this).toDp()
                            } + if (playerOpen || tabletLayoutOf()) 24.dp else if (hasPlayer) 156.dp else 92.dp,
                    )
                if (!onboarding && !fullscreen) AppUpdateHost()
                SharedLinkPlayer(vm, blocked = onboarding)
                AnimatedVisibility(
                    visible = onboarding,
                    enter = EnterTransition.None,
                    exit =
                        if (!motion.animate) ExitTransition.None
                        else fadeOut(motion.exit(280)) + scaleOut(motion.exit(320), targetScale = 1.04f),
                ) {
                    Onboarding(vm, dark) {
                        OnboardingStore.complete(activity)
                        onboarding = false
                    }
                }
            }
            }
            }
        }
    }
}

@Composable
internal fun Action(label: String, tag: String = "", enabled: Boolean = true, action: () -> Unit) =
    Pill(
        label,
        if (tag.isNotEmpty()) Modifier.testTag(tag) else Modifier,
        PillStyle.Primary,
        enabled,
        onClick = action,
    )

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun Panel(
    title: String,
    close: () -> Unit,
    full: Boolean = false,
    scroll: Boolean = true,
    swipeToClose: Boolean = true,
    page: Boolean = false,
    headerBack: (() -> Unit)? = null,
    headerBackTag: String = "",
    showHeader: Boolean = true,
    backdrop: Modifier = Modifier,
    enterFromBottom: Boolean = false,
    actions: @Composable RowScope.() -> Unit = {},
    content: @Composable ColumnScope.() -> Unit,
) {
    val motion = LocalMotion.current
    val height = with(LocalDensity.current) { LocalConfiguration.current.screenHeightDp.dp.toPx() }
    // Full-screen panels follow a downward drag and fly off when released past the threshold.
    // A player panel also rises the same way it leaves: it starts one screen down, slightly
    // shrunk, and settles into place, the exact reverse of the swipe-away.
    val rise = full && enterFromBottom && motion.animate
    val drag = remember { Animatable(if (rise) height else 0f) }
    if (rise) LaunchedEffect(Unit) { drag.animateTo(0f, motion.enter(460)) }
    // Tell the app shell while a full panel is pulled away, so the page behind can show.
    if (full) {
        LaunchedEffect(drag) { snapshotFlow { drag.value != 0f }.collect { PanelPull.active.value = it } }
        DisposableEffect(Unit) { onDispose { PanelPull.active.value = false } }
    }
    val dismissThreshold = with(LocalDensity.current) { 96.dp.toPx() }
    var leaving by remember { mutableStateOf(false) }
    val flyOff: suspend kotlinx.coroutines.CoroutineScope.(Int) -> Unit = flyOff@{
        if (leaving) return@flyOff
        leaving = true
        drag.animateTo(height, motion.exit(300))
        close()
    }
    val panelScope = rememberCoroutineScope()
    // Every way out of a full panel (header button, system Back) leaves like the swipe does.
    val animatedClose: () -> Unit = {
        if (full && motion.animate) panelScope.launch { flyOff(1) } else if (!leaving) close()
    }
    if (full && enterFromBottom) BackHandler(enabled = motion.animate && !leaving) { animatedClose() }
    @Composable
    fun Body(modifier: Modifier = Modifier, dismiss: () -> Unit = if (full) animatedClose else close) {
        Column(modifier.testTag("panel-body")) {
            if (showHeader && title.isNotBlank())
                Row(
                    Modifier.fillMaxWidth()
                        .heightIn(min = 56.dp)
                        .padding(horizontal = 8.dp)
                        .testTag("panel-header")
                        .swipeAxis(
                            enabled = full && swipeToClose,
                            horizontal = false,
                            offset = drag,
                            threshold = dismissThreshold,
                            allow = { it > 0 },
                            onSwipe = flyOff,
                        ),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    if (full || page)
                        MediaIconButton(
                            if (full) Glyph.Down else Glyph.Back,
                            if (full) "Minimize player"
                            else if (headerBack != null) "All settings" else "Close",
                            if (headerBack != null) headerBackTag else "panel-close",
                            action = headerBack ?: dismiss,
                        )
                    Text(
                        title,
                        Modifier.weight(1f).padding(horizontal = 8.dp),
                        style =
                            if (full) MaterialTheme.typography.titleMedium
                            else if (page) MaterialTheme.typography.headlineSmall
                            else MaterialTheme.typography.titleLarge,
                        textAlign = if (full) TextAlign.Center else TextAlign.Start,
                    )
                    actions()
                    if (full) Spacer(Modifier.size(48.dp))
                    // Sub-pages go back one level; only the top page and sheets close.
                    if (!full && !page) SheetClose(dismiss)
                }
            else if (!full && !page)
                // Untitled action sheets keep a close control without spending a header row.
                Box(Modifier.fillMaxWidth().height(0.dp).zIndex(1f)) {
                    Box(
                        Modifier.align(Alignment.TopEnd)
                            .wrapContentHeight(Alignment.Top, unbounded = true)
                            .padding(end = 8.dp)
                    ) {
                        SheetClose(dismiss)
                    }
                }
            Column(
                Modifier.weight(1f, fill = full || page)
                    .fillMaxWidth()
                    .then(if (scroll) Modifier.verticalScroll(rememberScrollState()) else Modifier)
                    .padding(
                        horizontal = if (full) 0.dp else MediaTokens.textInset,
                        vertical = if (full && !showHeader) 0.dp else 8.dp,
                    ),
                verticalArrangement =
                    Arrangement.spacedBy(
                        if (full || page) 16.dp else if (title.isBlank()) 0.dp else 8.dp
                    ),
                content = content,
            )
        }
    }
    if (full || page) {
        // Keep native video in the activity window; no new dialog surface/decoder handoff.
        Surface(
            Modifier.fillMaxSize()
                // The headerless video panel follows a drag too (YouTube-style minimize); its
                // video is a TextureView, which transforms with its layer.
                .then(
                    if (full && motion.animate)
                        Modifier.graphicsLayer {
                            compositingStrategy = CompositingStrategy.ModulateAlpha
                            val pulled = drag.value.coerceAtLeast(0f)
                            translationY = pulled
                            // The video panel stays opaque: it carries a live video texture.
                            if (showHeader)
                                alpha = 1f - (pulled / height).coerceIn(0f, 1f) * 0.5f
                            val shrink = 1f - (pulled / height).coerceIn(0f, 1f) * 0.08f
                            scaleX = shrink
                            scaleY = shrink
                        }
                    else Modifier
                )
                .semantics { paneTitle = title },
            color = MaterialTheme.colorScheme.background,
        ) {
            CompositionLocalProvider(
                LocalPanelDrag provides
                    if (full && swipeToClose) PanelDrag(drag, flyOff) else null
            ) {
                // The backdrop fills the whole window, under the status and gesture bars too;
                // only the content keeps clear of them.
                Box(Modifier.fillMaxSize().then(backdrop)) {
                    Body(Modifier.fillMaxSize().safeDrawingPadding().imePadding())
                }
            }
        }
    } else if (title == "About Vidcue") {
        MediaDialog(
            close,
            title,
            titleModifier = Modifier.testTag("panel-header"),
            actions = {
                Pill(
                    "Close",
                    Modifier.testTag("panel-close").semantics { contentDescription = "Close" },
                    PillStyle.Quiet,
                    onClick = close,
                )
            },
        ) {
            Column(
                Modifier.heightIn(max = LocalConfiguration.current.screenHeightDp.dp * 0.65f)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(16.dp),
                content = content,
            )
        }
    } else {
        val reduced = LocalReducedMotion.current
        val density = LocalDensity.current
        // Reduced motion must expose the sheet in its first layout, rather than waiting
        // below the window for ModalBottomSheet's asynchronous entrance to run.
        val state =
            if (reduced)
                remember(density) {
                    SheetState(
                        skipPartiallyExpanded = true,
                        density = density,
                        initialValue = SheetValue.Expanded,
                    )
                }
            else rememberModalBottomSheetState(skipPartiallyExpanded = true)
        val scope = rememberCoroutineScope()
        ModalBottomSheet(
            onDismissRequest = close,
            sheetState = state,
            shape = MediaTokens.sheet,
            containerColor =
                lerp(MaterialTheme.colorScheme.background, MaterialTheme.colorScheme.surface, 0.6f),
            contentColor = MaterialTheme.colorScheme.onSurface,
            tonalElevation = 0.dp,
            scrimColor = Color.Black.copy(alpha = 0.62f),
            dragHandle = {
                Box(
                    Modifier.padding(top = 10.dp, bottom = 4.dp)
                        .size(width = 40.dp, height = 4.dp)
                        .background(
                            MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.35f),
                            MediaTokens.pill,
                        )
                )
            },
        ) {
            Body(
                Modifier.fillMaxWidth()
                    .heightIn(max = LocalConfiguration.current.screenHeightDp.dp * 0.9f)
                    .imePadding(),
                dismiss = {
                    if (reduced) close()
                    else
                        scope.launch {
                            state.hide()
                            close()
                        }
                },
            )
        }
    }
}

/** True while a full-screen panel is being dragged or flung away. */
internal object PanelPull {
    val active = mutableStateOf(false)
}

/** Slide-in side for the next BrowsePane after a tab swipe (main thread only). */
private var pendingTabEntry = 0

/**
 * Sideways swipes anywhere on the page move between the bottom destinations in their display
 * order (Home, Music, Radio, Movies, Downloads). The page follows the finger; rows that scroll sideways
 * themselves (filter chips, shelves) keep their own drags.
 */
@Composable
private fun Modifier.tabSwipe(tab: Int, enabled: Boolean, showMovies: Boolean, choose: (Int) -> Unit): Modifier {
    val motion = LocalMotion.current
    val density = LocalDensity.current
    val width = with(density) { LocalConfiguration.current.screenWidthDp.dp.toPx() }
    val pull = remember { Animatable(0f) }
    val current by rememberUpdatedState(tab)
    val select by rememberUpdatedState(choose)
    return this.swipeAxis(
            enabled = enabled,
            horizontal = true,
            offset = pull,
            threshold = with(density) { 96.dp.toPx() },
            // Dragging left (negative) moves forward.
            allow = { neighbourTab(current, -it, showMovies) != null },
            onSwipe = { direction ->
                val next = neighbourTab(current, -direction, showMovies) ?: return@swipeAxis
                pull.animateTo(direction * width * 0.35f, motion.exit(160))
                pendingTabEntry = -direction
                select(next)
                pull.snapTo(0f)
            },
        )
        .graphicsLayer {
            val dragged = (abs(pull.value) / width).coerceIn(0f, 1f)
            translationX = pull.value * 0.6f
            alpha = 1f - dragged * 0.6f
        }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun BrowsePane(
    vm: YtuiViewModel,
    tab: Int,
    onAccount: () -> Unit,
    onFeeds: () -> Unit,
    searchActive: Boolean,
    onSearchFocus: (Boolean) -> Unit,
    releaseSearchFocus: () -> Unit,
    browseFocus: FocusRequester,
    scrollToTop: Int = 0,
    /** Library pages: no search, and [chips] replaces the filter row. */
    library: Boolean = false,
    chips: (@Composable () -> Unit)? = null,
) {
    // Sheets outlive recycled grid items, including relayout while the IME closes.
    var cardMenu by remember { mutableStateOf<Pair<MediaCard, Boolean>?>(null) }
    val openCardMenu: (MediaCard, Boolean) -> Unit = { media, square ->
        releaseSearchFocus()
        cardMenu = media to square
    }
    val feed by vm.feeds[tab].collectAsState()
    val shared by vm.sharedQuery.collectAsState()
    val mobile by vm.mobile.collectAsState()
    val config by vm.config.collectAsState()
    // Collected once per pane, not once per card.
    val thumbnails = config.optBoolean("thumbnails", true)
    val reduced = LocalReducedMotion.current
    val account by vm.account.collectAsState()
    val connected = account.optBoolean("connected")
    val history by vm.recent.collectAsState()
    val wideSearch =
        tab == 1 &&
            LocalConfiguration.current.screenWidthDp >= 840 &&
            SearchRequests.isSearch(feed.request)
    val motion = LocalMotion.current
    val density = LocalDensity.current
    // A committed tab swipe (handled around the whole content area) slides the next tab in
    // from the side it was pulled from.
    val entry = remember { Animatable(if (motion.animate && !feed.instantContent) 0f else 1f) }
    // A tab swipe recreates this pane (key(tab)); the direction is handed over once.
    var enterFrom by remember { mutableIntStateOf(pendingTabEntry.also { pendingTabEntry = 0 }) }
    LaunchedEffect(tab, feed.request, reduced, feed.instantContent) {
        // Plain feed changes are animated by the cards themselves (appearance); only a tab
        // swipe slides the whole grid, so the grid is not faded a second time.
        if (!motion.animate || enterFrom == 0 || feed.instantContent) entry.snapTo(1f)
        else {
            entry.snapTo(0f)
            entry.animateTo(1f, motion.enter(if (enterFrom == 0) 260 else 340))
        }
        enterFrom = 0
    }
    // Cards animate in once per loaded list, not again when scrolled back into view.
    val seen = remember(tab, feed.request) { mutableSetOf<Any>() }
    var gridWidth by remember { mutableIntStateOf(0) }
    val cardIndex =
        remember(feed.cards) { feed.cards.withIndex().associate { it.value.key to it.index } }
    var query by rememberSaveable(tab) { mutableStateOf("") }
    val keyboard = LocalSoftwareKeyboardController.current
    val searchResults = !library && SearchRequests.isSearch(feed.request)
    val searchMode = !library && (searchActive || searchResults)
    LaunchedEffect(feed.request) { SearchRequests.query(feed.request)?.let { query = it } }
    // Pages opened from another page (playlists, albums, channels) get a header; lists of results
    // and library feeds read as rows.
    val collectionPage = SubPages.collectionId(feed.request) != null
    val compactList = tab == 0 || (tab != 3 && (searchResults || (collectionPage && tab == 1)))
    val numberedTracks =
        collectionPage && vm.collectionCard(SubPages.collectionId(feed.request)!!)?.kind == "album"
    // Side inset of full-width rows: Music and Radio grids already pad their content.
    val sideInset = if (tab in 2..3) 0.dp else MediaTokens.textInset
    val homeTopic = if (tab == 1) SearchRequests.topicName(feed.request) else null
    val atHome = (feed.request == "Home" || homeTopic != null) && !searchResults
    // Signed out, YouTube's Home is empty: show what is popular this week instead of an empty page.
    LaunchedEffect(tab, connected, feed.request, feed.loading, feed.cards.isEmpty(), feed.generation) {
        if (tab == 1 && !connected && feed.request == "Home" && !feed.loading && feed.generation > 0L &&
            feed.cards.isEmpty() && feed.error == null)
            vm.navigate(1, SearchRequests.topic(SearchRequests.topics.first()), remember = false)
    }
    val resumeVideo =
        if (tab == 1 && atHome) history.firstOrNull { it.kind == "video" && !it.live && it.url.isNotBlank() }
        else null
    val recentSongs =
        if (tab == 2 && atHome) history.filter { it.audio && it.url.isNotBlank() }.distinctBy { it.key }.take(6)
        else emptyList()
    val leadItems =
        (if (resumeVideo != null) 1 else 0) +
            (if (recentSongs.size >= 2) 1 else 0) +
            (if (collectionPage) 1 else 0)
    val focus = remember { FocusRequester() }
    // Dismissing the keyboard from an empty, unsubmitted search leaves search mode. Otherwise
    // the page title, Settings and More stay hidden behind an empty field (most visibly at
    // large text sizes, where a second Back is easy to miss). A typed query keeps its session.
    val imeShown = WindowInsets.ime.getBottom(density) > 0
    var imeWasShown by remember { mutableStateOf(false) }
    LaunchedEffect(imeShown) {
        if (imeShown) imeWasShown = true
        else if (imeWasShown) {
            imeWasShown = false
            if (searchActive && query.isBlank() && !searchResults) releaseSearchFocus()
        }
    }
    val context = androidx.compose.ui.platform.LocalContext.current
    val rememberSearches = config.optBoolean("playback_history", true)
    val grid = rememberLazyGridState(initialFirstVisibleItemIndex = feed.scrollIndex)
    // Re-selecting the current tab scrolls to the top; a freshly shown pane keeps
    // its restored position.
    val initialScrollToTop = remember { scrollToTop }
    // The search bar and filters move with the scroll: they slide up as the feed scrolls down
    // and come back with any upward scroll. They float over the grid and only their layer moves,
    // so hiding them never re-measures the grid mid-scroll.
    var headerHeight by remember { mutableIntStateOf(0) }
    val headerOffset = remember(tab) { mutableFloatStateOf(0f) }
    val headerScroll =
        remember(headerOffset) {
            object : NestedScrollConnection {
                override fun onPreScroll(available: Offset, source: NestedScrollSource): Offset {
                    val dy = available.y
                    if (dy != 0f)
                        headerOffset.floatValue =
                            (headerOffset.floatValue - dy).coerceIn(0f, headerHeight.toFloat())
                    return Offset.Zero
                }
            }
        }
    LaunchedEffect(searchActive, feed.request) { headerOffset.floatValue = 0f }
    LaunchedEffect(scrollToTop) {
        headerOffset.floatValue = 0f
        if (scrollToTop != initialScrollToTop) {
            if (reduced) grid.scrollToItem(0) else grid.animateScrollToItem(0)
        }
    }
    LaunchedEffect(grid) {
        snapshotFlow { grid.firstVisibleItemIndex }
            .distinctUntilChanged()
            .collect { vm.rememberScroll(tab, it) }
    }
    val focused by vm.playerOpen.collectAsState()
    val playingUrl by
        remember(vm) { vm.player.map { it.media?.optString("url") }.distinctUntilChanged() }
            .collectAsState(null)
    val playingNow by
        remember(vm) { vm.player.map { it.playing }.distinctUntilChanged() }.collectAsState(false)
    val cardsByKey = remember(feed.cards) { feed.cards.associateBy { it.key } }
    // Provider sections ("Quick picks", "Listen again") become titled shelves once a feed has
    // at least two of them; search results and single-section feeds stay a grid.
    val shelves =
        remember(feed.cards, tab, feed.request) {
            val searching = (feed.request as? JSONObject)?.has("Search") == true
            if (tab !in 1..2 || searching) null
            else
                feed.cards
                    .groupBy {
                        it.value.optString("section").takeUnless { s -> s.isBlank() || s == "null" } ?: ""
                    }
                    .takeIf { groups -> groups.keys.count { it.isNotBlank() } >= 2 }
                    ?.entries
                    ?.map { it.key to it.value }
        }
    LaunchedEffect(grid, feed.cards, focused) {
        if (!focused)
            snapshotFlow {
                    val visible =
                        grid.layoutInfo.visibleItemsInfo.flatMap { info ->
                            cardsByKey[info.key]?.let(::listOf)
                                ?: shelves
                                    ?.firstOrNull { "shelf:${it.first}" == info.key }
                                    ?.second
                                    ?.take(2)
                                ?: emptyList()
                        }
                    visible.take(3)
                }
                .distinctUntilChanged()
                .collectLatest { cards ->
                    delay(400)
                    vm.prepareVisible(cards)
                }
    }
    LaunchedEffect(feed.generation) {
        if (grid.firstVisibleItemIndex != feed.scrollIndex) grid.scrollToItem(feed.scrollIndex)
    }
    LaunchedEffect(shared) {
        // YouTube links are played by SharedLinkPlayer; other shared text is a search.
        if (shared.isNotBlank() && SharedLinks.mediaLink(shared) == null) {
            query = shared
            vm.sharedQuery.value = ""
        }
    }
    // Include the full-width headers, and wait for this page to be measured before
    // using layout indices (the preceding page or skeleton may still be laid out).
    // An empty page explains itself once: its notice becomes the empty state's text instead of
    // a separate line above a second, generic message.
    val noticeInEmptyState =
        feed.notice != null && !feed.loading && feed.cards.isEmpty() && feed.error == null
    val pageItemCount =
        (shelves?.let { it.size * 2 } ?: feed.cards.size) +
            leadItems + (if (feed.notice != null && !noticeInEmptyState) 1 else 0)
    // A page that arrived empty but says more exists (a shell whose shelves load separately)
    // fetches its next part once, instead of waiting for a scroll that cannot happen.
    var emptyFollowed by remember(tab, feed.request) { mutableStateOf(false) }
    LaunchedEffect(feed.cards.isEmpty(), feed.continuation, feed.loading, feed.error) {
        if (
            feed.cards.isEmpty() &&
                feed.continuation != null &&
                !feed.loading &&
                feed.error == null &&
                !emptyFollowed
        ) {
            emptyFollowed = true
            vm.more()
        }
    }
    LaunchedEffect(grid, feed.continuation, feed.loading, feed.error, feed.cards.size, pageItemCount) {
        if (
            feed.cards.isNotEmpty() &&
                feed.continuation != null &&
                !feed.loading &&
                feed.error == null
        ) {
            snapshotFlow {
                    val layout = grid.layoutInfo
                    val last = layout.visibleItemsInfo.lastOrNull()?.index
                    // A Music shelf contains many songs but occupies only two grid items
                    // (heading + row). Compare indices with the laid-out item count.
                    last != null &&
                        layout.totalItemsCount == pageItemCount &&
                        last >= layout.totalItemsCount - 5
                }
                .distinctUntilChanged()
                .collect { nearEnd -> if (nearEnd) vm.more() }
        }
    }
    val headerSpace = with(density) { headerHeight.toDp() }
    Box(Modifier.fillMaxSize().clipToBounds().testTag("browse-pane")) {
        // Pull to refresh reuses the overflow menu's "Refresh feed" request.
        var refreshing by remember { mutableStateOf(false) }
        LaunchedEffect(feed.loading) { if (!feed.loading) refreshing = false }
        val pullState = androidx.compose.material3.pulltorefresh.rememberPullToRefreshState()
        Box(Modifier.fillMaxSize().nestedScroll(headerScroll)) {
        PullToRefreshBox(
            refreshing && feed.loading,
            onRefresh = {
                refreshing = true
                vm.navigate(request = feed.request, remember = false)
            },
            state = pullState,
            indicator = {
                Box(Modifier.matchParentSize().padding(top = headerSpace)) {
                    RefreshDisc(pullState, refreshing && feed.loading)
                }
            },
            modifier =
                Modifier.fillMaxSize().pointerInput(searchActive) {
                    // Touching content ends an unsubmitted search session, as tapping
                    // outside a field does elsewhere. The touch itself is not consumed.
                    if (searchActive)
                        awaitEachGesture {
                            awaitFirstDown(
                                requireUnconsumed = false,
                                pass = androidx.compose.ui.input.pointer.PointerEventPass.Initial,
                            )
                            releaseSearchFocus()
                        }
                },
        ) {
            val fixedColumns = tab in 2..3 && LocalConfiguration.current.screenWidthDp < 600
            val minCell = if (tab in 2..3) 164.dp else 280.dp
            val sidePadding = if (tab in 2..3) 16.dp else 0.dp
            val cellGap = if (tab in 2..3) 12.dp else 8.dp
            // Cover width in pixels, as the grid lays its cells out; covers use it instead of
            // measuring themselves.
            val cellPixels =
                remember(gridWidth, fixedColumns, minCell, density) {
                    with(density) {
                        val available = gridWidth - 2 * sidePadding.roundToPx()
                        val gap = cellGap.roundToPx()
                        val count =
                            if (fixedColumns) 2
                            else maxOf(1, (available + gap) / (minCell.roundToPx() + gap))
                        if (gridWidth <= 0) 0 else ((available - gap * (count - 1)) / count).coerceAtLeast(1)
                    }
                }
            // Covers read these once from the grid's composition instead of every card
            // providing its own copies.
            val scrollingProbe = remember(grid) { { grid.isScrollInProgress } }
            CompositionLocalProvider(
                LocalArtworkScrolling provides scrollingProbe,
                LocalArtworkPixels provides if (wideSearch) 0 else cellPixels,
            ) {
            LazyVerticalGrid(
                columns =
                    if (fixedColumns) GridCells.Fixed(2)
                    else GridCells.Adaptive(minCell),
                modifier =
                    Modifier.fillMaxSize()
                        .onSizeChanged { gridWidth = it.width }
                        .graphicsLayer {
                            // No offscreen buffer for a screen-sized scrolling grid.
                            compositingStrategy = CompositingStrategy.ModulateAlpha
                            val settle = if (feed.instantContent) 0f else 1f - entry.value
                            alpha = 1f - settle.coerceIn(0f, 1f)
                            translationX = settle * enterFrom * 64.dp.toPx()
                        }
                        .testTag("feed-list")
                        .focusRequester(browseFocus)
                        .focusable(),
                state = grid,
                contentPadding =
                    PaddingValues(
                        start = if (tab in 2..3) 16.dp else 0.dp,
                        end = if (tab in 2..3) 16.dp else 0.dp,
                        top = 4.dp + headerSpace,
                        bottom = 24.dp + LocalBottomOverlay.current,
                    ),
                horizontalArrangement = Arrangement.spacedBy(if (tab in 2..3) 12.dp else 8.dp),
                // Lists of rows sit close together, as in music apps; cards and shelves breathe.
                verticalArrangement =
                    Arrangement.spacedBy(
                        when {
                            compactList || collectionPage -> 4.dp
                            tab in 2..3 -> 20.dp
                            else -> 28.dp
                        }
                    ),
            ) {
                if (collectionPage)
                    item(span = { GridItemSpan(maxLineSpan) }, key = "collection-header", contentType = "collection-header") {
                        CollectionHeader(vm, feed, tab, thumbnails, inset = sideInset, modifier = Modifier.padding(bottom = 12.dp))
                    }
                if (resumeVideo != null)
                    item(span = { GridItemSpan(maxLineSpan) }, key = "continue", contentType = "continue") {
                        CompositionLocalProvider(LocalArtworkPixels provides 0) {
                            ContinueHero(resumeVideo, thumbnails) { vm.play(resumeVideo, resume = true) }
                        }
                    }
                if (recentSongs.size >= 2)
                    item(span = { GridItemSpan(maxLineSpan) }, key = "recent", contentType = "recent") {
                        CompositionLocalProvider(LocalArtworkPixels provides 0) {
                            RecentTiles("Jump back in", recentSongs, thumbnails, inset = sideInset) { vm.play(it) }
                        }
                    }
                if (feed.notice != null && !noticeInEmptyState)
                    item(span = { GridItemSpan(maxLineSpan) }) {
                        Text(
                            feed.notice!!,
                            Modifier.padding(horizontal = MediaTokens.textInset),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                if (feed.error != null)
                    item(span = { GridItemSpan(maxLineSpan) }) {
                        val needsAccount =
                            feed.errorCode in listOf("auth_required", "consent_required")
                        EmptyMediaState(
                            if (needsAccount) Glyph.Account else Glyph.Info,
                            if (needsAccount) "Sign in to see this" else "This page didn't load",
                            // The engine's account messages point at menus by name; on this
                            // screen the button below is the way there.
                            if (needsAccount)
                                "Sign in to YouTube, or import a session from another browser, " +
                                    "to open this page."
                            else feed.error!!,
                        ) {
                            FlowRow(
                                horizontalArrangement =
                                    Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
                                verticalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                if (needsAccount) {
                                    Action("Open account", "feed-account-action") {
                                        onAccount()
                                    }
                                    QuietAction("Try again") { vm.retryFeed() }
                                } else Action("Try again") { vm.retryFeed() }
                            }
                            Text(
                                "You can still search or open a video link.",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                if (shelves != null)
                    for ((section, cards) in shelves) {
                        val heading = section.ifBlank { "More for you" }
                        // Albums and playlists open as their own pages, so a shelf of only
                        // collections or live entries has nothing to play in order.
                        val canPlayAll = cards.any { !it.collection && !it.live && it.url.isNotBlank() }
                        item(
                            span = { GridItemSpan(maxLineSpan) },
                            key = "shelf-head:$section",
                            contentType = "shelf-head",
                        ) {
                            ShelfHeading(
                                heading,
                                Modifier.testTag("shelf-$section"),
                                inset = sideInset,
                                action = if (canPlayAll) "Play all" else null,
                                actionTag = "shelf-play-all-$section",
                            ) {
                                vm.playAll(cards)
                            }
                        }
                        item(
                            span = { GridItemSpan(maxLineSpan) },
                            key = "shelf:$section",
                            contentType = if (cards.all { it.isSongRow(tab) }) "song-shelf" else "shelf",
                        ) {
                            if (cards.all { it.isSongRow(tab) })
                                CompositionLocalProvider(LocalArtworkPixels provides 0) {
                                    QuickPicksShelf(
                                        cards,
                                        thumbnails,
                                        mobile.optBoolean("show_channel", true),
                                        playingUrl,
                                        playingNow,
                                        onPlay = { vm.play(it) },
                                        onMenu = { openCardMenu(it, true) },
                                        // In a padded grid (Music, Radio) shelves still scroll to
                                        // the screen edges; their first item keeps its place.
                                        inset = if (tab in 2..3) 20.dp else sideInset,
                                        modifier = if (tab in 2..3) Modifier.bleedHorizontally(16.dp) else Modifier,
                                    )
                                }
                            else
                            LazyRow(
                                if (tab in 2..3) Modifier.fillMaxWidth().bleedHorizontally(16.dp) else Modifier.fillMaxWidth(),
                                contentPadding = PaddingValues(horizontal = if (tab in 2..3) 16.dp else sideInset),
                                horizontalArrangement = Arrangement.spacedBy(12.dp),
                            ) {
                                items(cards, key = { it.key }, contentType = { "shelf-card" }) { media ->
                                    val shelfWidth = if (tab == 1) 272.dp else 152.dp
                                    CompositionLocalProvider(
                                        LocalArtworkPixels provides with(density) { shelfWidth.roundToPx() }
                                    ) {
                                        Box(
                                            Modifier.width(shelfWidth).pressFeedback()
                                        ) {
                                            val current = media.url.isNotBlank() && media.url == playingUrl
                                            MediaRow(
                                                vm,
                                                media,
                                                tab,
                                                thumbnails,
                                                mobile,
                                                current,
                                                current && playingNow,
                                                shelf = true,
                                                openMenu = openCardMenu,
                                            )
                                        }
                                    }
                                }
                            }
                        }
                    }
                else items(
                    feed.cards,
                    key = { it.key },
                    span = {
                        GridItemSpan(
                            if (wideSearch || compactList || it.isSongRow(tab)) maxLineSpan else 1
                        )
                    },
                    contentType = {
                        when {
                            wideSearch -> "wide-row"
                            it.isSongRow(tab) -> "song-row"
                            compactList -> "compact-row"
                            else -> "media-card"
                        }
                    },
                ) { media ->
                    // Scroll state is read in image effects, without recomposing every card.
                    run {
                        Box(
                            Modifier.then(
                                    // Cached categories display at once, without card entrances.
                                    if (feed.instantContent) Modifier
                                    else
                                        Modifier.appearance(
                                            media.key,
                                            cardIndex[media.key] ?: 0,
                                            seen,
                                            isScrolling = { grid.isScrollInProgress },
                                        )
                                )
                                .pressFeedback()
                        ) {
                            val current = media.url.isNotBlank() && media.url == playingUrl
                            if (wideSearch) WideMediaRow(vm, media, thumbnails, mobile, openCardMenu)
                            else if (media.isSongRow(tab))
                                CompositionLocalProvider(LocalArtworkPixels provides 0) {
                                    SongRow(
                                        media,
                                        thumbnails,
                                        mobile.optBoolean("show_channel", true),
                                        current,
                                        current && playingNow,
                                        onPlay = { vm.play(media) },
                                        onMenu = { openCardMenu(media, true) },
                                        number =
                                            if (numberedTracks) (cardIndex[media.key] ?: 0) + 1 else null,
                                    )
                                }
                            else if (compactList)
                                CompositionLocalProvider(LocalArtworkPixels provides 0) {
                                    CompactMediaRow(
                                        media,
                                        thumbnails,
                                        mobile,
                                        current,
                                        current && playingNow,
                                        onPlay = { vm.play(media) },
                                        onMenu = { openCardMenu(media, media.audio || media.kind == "album") },
                                        inset = sideInset,
                                        music = tab == 2,
                                    )
                                }
                            else
                                MediaRow(
                                    vm, media, tab, thumbnails, mobile, current,
                                    current && playingNow, openMenu = openCardMenu,
                                )
                        }
                    }
                }
                if (feed.loading && feed.cards.isEmpty() && feed.error == null)
                    items(if (tab in 2..3) 6 else 3, contentType = { "skeleton" }) {
                        FeedSkeleton(square = tab in 2..3)
                    }
                else if (feed.loading)
                    item(span = { GridItemSpan(maxLineSpan) }) {
                        Box(
                            Modifier.fillMaxWidth().padding(24.dp).semantics {
                                contentDescription = "Loading…"
                            },
                            contentAlignment = Alignment.Center,
                        ) {
                            MediaSpinner()
                        }
                    }
                if (!feed.loading && feed.cards.isEmpty() && feed.error == null)
                    item(span = { GridItemSpan(maxLineSpan) }) {
                        // Signed out, Home has no recommendations: offer topics and sign-in
                        // instead of an empty page.
                        if (tab in 1..2 && atHome && !connected)
                            QuickStart(onTopic = { vm.navigate(1, SearchRequests.topic(it), remember = false) }, onSignIn = onAccount)
                        else
                        EmptyMediaState(
                            if (tab == 2) Glyph.Music
                            else if (tab == 3) Glyph.Radio
                            else if (tab == 0) Glyph.Library else Glyph.Search,
                            if (searchResults) "No results" else "Nothing here yet",
                            if (searchResults) "Try different words, or another filter."
                            else feed.notice
                                ?: when (tab) {
                                    3 -> "Search for a station, or browse by country or genre."
                                    0 -> "What you save, like or add to a playlist appears here."
                                    else -> "Search for something to watch, or sign in for a Home made for you."
                                },
                        ) {
                            if (tab == 3) Action("Browse stations") { onFeeds() }
                            else if (tab != 0 && !searchResults) Action("Search") { onSearchFocus(true) }
                        }
                    }
            }
            }
        }
        // While the field is focused and empty, recent searches cover the feed (which stays
        // composed underneath so focus and scroll position survive).
        androidx.compose.animation.AnimatedVisibility(
            searchActive && query.isBlank(),
            Modifier.matchParentSize(),
            enter = fadeIn(motion.fade(160)),
            exit = fadeOut(motion.fade(120)),
        ) {
            SearchSuggestions(
                radio = tab == 3,
                remember = rememberSearches,
                onSearch = {
                    query = it
                    keyboard?.hide()
                    if (rememberSearches) RecentSearches.add(context, tab == 3, it)
                    vm.search(it)
                },
                onPaste = {
                    query = it
                    keyboard?.hide()
                    if (rememberSearches) RecentSearches.add(context, tab == 3, it)
                    vm.search(it)
                },
                modifier = Modifier.fillMaxSize().padding(top = headerSpace),
            )
        }
        // While typing, suggestions cover the page: the text as typed, then earlier searches
        // and titles you played or can see that contain it.
        val submitted = SearchRequests.query(feed.request)
        androidx.compose.animation.AnimatedVisibility(
            searchActive && query.isNotBlank() && query.trim() != submitted,
            Modifier.matchParentSize(),
            enter = fadeIn(motion.fade(120)),
            exit = fadeOut(motion.fade(100)),
        ) {
            val recentSearches = remember(searchActive) { RecentSearches.load(context, tab == 3) }
            val names =
                remember(feed.cards, history) {
                    history.flatMap { listOf(it.title, it.creator) } +
                        feed.cards.flatMap { listOf(it.title, it.creator) }
                }
            TypingSuggestions(
                query,
                recentSearches,
                names,
                onPick = { text ->
                    query = text
                    keyboard?.hide()
                    if (rememberSearches) RecentSearches.add(context, tab == 3, text)
                    vm.search(text)
                },
                onFill = { query = it },
                modifier = Modifier.fillMaxSize().padding(top = headerSpace),
            )
        }
        }
        // Chips belong to top-level pages; a playlist or channel page has its own header.
        val subpage = SubPages.isSubpage(feed.request)
        val filters = if (searchResults) SearchRequests.filters(tab) else emptyList()
        // A lone "For you" chip on For you itself chooses nothing (signed out, no categories).
        val loneSource = tab == 2 && feed.request == "Home" && feed.categories.isEmpty()
        val showChips =
            !searchMode && !subpage && !loneSource && (chips != null || tab in 1..3 || feed.categories.isNotEmpty())
        Column(
            Modifier.fillMaxWidth()
                .onSizeChanged { headerHeight = it.height }
                .graphicsLayer { translationY = if (searchActive) 0f else -headerOffset.floatValue }
                .background(MaterialTheme.colorScheme.background)
                .padding(bottom = if (searchMode || showChips) 12.dp else 0.dp)
                .testTag("browse-header"),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
        // Search is opened from the page header's search button; the field shows only then.
        if (searchMode) {
            var fieldHadFocus by remember { mutableStateOf(false) }
            Row(
                Modifier.align(Alignment.CenterHorizontally)
                    .padding(horizontal = 8.dp)
                    .widthIn(max = 720.dp)
                    .fillMaxWidth()
                    .padding(top = 8.dp)
                    .testTag("search-row"),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                MediaIconButton(Glyph.Back, "Leave search") {
                    releaseSearchFocus()
                    if (searchResults) vm.back()
                }
                SearchField(
                    query,
                    { query = it },
                    if (tab == 3) "Search stations" else "Search",
                    // A single-line input should not stretch across a wide canvas;
                    // cap it and center it when the pane is wider than the cap.
                    Modifier.weight(1f)
                        .padding(end = 8.dp)
                        .focusRequester(focus)
                        .onFocusChanged {
                            // The first report is "not focused" before focus is requested;
                            // only a real loss of focus ends the search session.
                            if (it.isFocused) {
                                fieldHadFocus = true
                                onSearchFocus(true)
                            } else if (fieldHadFocus) {
                                fieldHadFocus = false
                                onSearchFocus(false)
                            }
                        }
                        .testTag("search-field"),
                    searching = true,
                    onClear = {
                        query = ""
                        focus.requestFocus()
                        keyboard?.show()
                    },
                    onSubmit = {
                        if (rememberSearches) RecentSearches.add(context, tab == 3, query)
                        vm.search(query)
                        // Results take over: no blinking cursor or keyboard over them.
                        releaseSearchFocus()
                    },
                )
            }
            // Opened from the search button: take focus and bring up the keyboard.
            LaunchedEffect(Unit) {
                if (searchActive && !searchResults) {
                    runCatching { focus.requestFocus() }
                    keyboard?.show()
                }
            }
        }
        if (filters.isNotEmpty())
            FadingLazyRow(
                Modifier.fillMaxWidth().testTag("search-filters"),
                contentPadding = PaddingValues(horizontal = MediaTokens.textInset),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                items(filters) { label ->
                    MediaChip(
                        label,
                        selected = SearchRequests.filter(feed.request) == label,
                        modifier = Modifier.testTag("search-filter-$label"),
                    ) {
                        val text = SearchRequests.query(feed.request) ?: query
                        vm.navigate(request = SearchRequests.request(tab, text, label), remember = false)
                    }
                }
            }
        else if (showChips && chips != null) chips()
        else if (showChips)
            FadingLazyRow(
                Modifier.fillMaxWidth().padding(top = 4.dp).testTag("feed-filters"),
                contentPadding = PaddingValues(horizontal = MediaTokens.textInset),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                // Signed out, Home's chips are topics: each shows what is popular this week.
                if (tab == 1 && !connected)
                    items(SearchRequests.topics) { topic ->
                        MediaChip(
                            topic,
                            selected = homeTopic == topic,
                            modifier = Modifier.testTag("topic-$topic"),
                        ) {
                            vm.navigate(1, SearchRequests.topic(topic), remember = false)
                        }
                    }
                else if (tab in 1..2)
                    items(
                        // Your own playlists, likes and saves live in Library; Home keeps the
                        // two feeds people switch between.
                        listOfNotNull(
                            "For you" to "Home",
                            if (tab == 1 && connected) "Subscriptions" to "Subscriptions" else null,
                        )
                    ) { (label, request) ->
                        MediaChip(
                            label,
                            selected = feed.request == request,
                            modifier = Modifier.testTag("source-$request"),
                        ) {
                            vm.navigate(tab, request)
                        }
                    }
                if (tab == 3) {
                    items(listOf("Featured" to "Cliamp", "Saved" to "Favorites")) { (label, request) ->
                        MediaChip(
                            label,
                            selected = feed.request == request,
                            modifier = Modifier.testTag("source-$request"),
                        ) {
                            vm.navigate(tab, request, remember = false)
                        }
                    }
                    item {
                        MediaChip("Countries and genres", modifier = Modifier.testTag("feeds-button")) { onFeeds() }
                    }
                }
                if (homeTopic == null) items(feed.categories) { category ->
                    MediaChip(
                        category.optString("title"),
                        modifier = Modifier.testTag("category-${category.optString("title")}"),
                        selected =
                            feed.request !in listOf("Home", "Subscriptions") &&
                                category.opt("request")?.toString() == feed.request.toString(),
                    ) {
                        vm.navigate(request = category.get("request"))
                    }
                }
            }
        }
    }
    cardMenu?.let { (media, square) ->
        MediaCardMenu(vm, media, thumbnails, square) { cardMenu = null }
    }
}

private val LocalArtworkScrolling = staticCompositionLocalOf<() -> Boolean> { { false } }

@Composable
internal fun Artwork(
    media: MediaCard,
    modifier: Modifier,
    enabled: Boolean = true,
    shape: androidx.compose.ui.graphics.Shape = MediaTokens.small,
) {
    val frame =
        modifier
            .clip(shape)
            .background(MaterialTheme.colorScheme.surfaceContainer)
            .testTag("artwork-frame-${media.key}")
    // Feed grids say how wide their cells are, so a card needs no subcomposition of its own
    // (one per cover cost scroll frames). Elsewhere the size comes from the constraints, not
    // onSizeChanged: lazy lists reuse layout nodes, and a reused node of unchanged size never
    // reports its size again.
    val hinted = LocalArtworkPixels.current
    if (hinted > 0)
        Box(frame) { ArtworkBody(media, ArtworkCache.decodeSize(hinted), enabled) }
    else
        BoxWithConstraints(frame) {
            // A shared decode size: a one-pixel width difference between grid columns must not
            // restart the load or fetch and cache the same cover again.
            val pixels = with(LocalDensity.current) { ArtworkCache.decodeSize(maxWidth.roundToPx()) }
            ArtworkBody(media, pixels, enabled)
        }
}

/** Width in pixels of the artwork cells a feed grid lays out; 0 when unknown. */
internal val LocalArtworkPixels = staticCompositionLocalOf { 0 }

@Composable
private fun BoxScope.ArtworkBody(media: MediaCard, pixels: Int, enabled: Boolean) {
    run {
        val source = media.artworkSource
        // A memory-cache hit paints in the first frame (no placeholder flash on scroll-back).
        val bitmapState =
            remember(media.key, source, pixels, enabled) {
                mutableStateOf(if (enabled) ArtworkCache.cached(media, pixels) else null)
            }
        val bitmap by bitmapState
        val scrolling = LocalArtworkScrolling.current
        // Key the producer by the exact cell it fills so relayout cannot leave a new
        // placeholder attached to work publishing into an abandoned state cell.
        LaunchedEffect(bitmapState) {
            if (bitmapState.value != null || !enabled || !ArtworkCache.hasArtwork(media)) return@LaunchedEffect
            // Retry transient failures with a growing pause outside the limited decode slots.
            // Leaving the screen cancels retries and work that has not started yet.
            if (BuildConfig.DEBUG)
                android.util.Log.d("ytui.artwork", "Loading cover ${media.key.hashCode()} at $pixels px")
            var pause = 1_000L
            repeat(8) {
                // Let flings use the frame budget for scrolling. A small settling window avoids
                // starting decodes between consecutive drags; cached art still paints immediately.
                while (scrolling()) {
                    snapshotFlow { scrolling() }.first { !it }
                    delay(100)
                }
                val loaded =
                    try {
                        ArtworkCache.load(media, pixels)
                    } catch (cancelled: kotlinx.coroutines.CancellationException) {
                        if (BuildConfig.DEBUG)
                            android.util.Log.d("ytui.artwork", "Cover ${media.key.hashCode()} detached at $pixels px")
                        throw cancelled
                    } catch (error: Exception) {
                        null
                    }
                if (loaded != null) {
                    bitmapState.value = loaded
                    if (BuildConfig.DEBUG)
                        android.util.Log.d("ytui.artwork", "Cover ${media.key.hashCode()} ready at $pixels px")
                    return@LaunchedEffect
                }
                android.util.Log.w("ytui.artwork", "Cover not loaded; retrying in $pause ms")
                delay(pause)
                pause = (pause * 2).coerceAtMost(15_000L)
            }
        }
        // While the exact size or a new source loads, keep painting the last cover of the same
        // item (for example after Next, or a larger size for the full player) instead of
        // flashing the placeholder.
        val shown = bitmap ?: if (enabled) ArtworkCache.recent(media) else null
        // Cached art paints at once; art that arrives later fades in over the placeholder.
        val motion = LocalMotion.current
        val reveal =
            remember(media.key, source, pixels, enabled) {
                Animatable(if (shown != null || !motion.animate) 1f else 0f)
            }
        LaunchedEffect(reveal, bitmap, motion) {
            if (bitmap != null && reveal.value < 1f) reveal.animateTo(1f, motion.fade(180))
        }
        // Keep a quiet, fixed-size surface underneath the fade instead of removing the icon
        // and briefly exposing a different background as soon as the image arrives.
        Box(
            Modifier.fillMaxSize()
                .graphicsLayer { alpha = if (shown == null) 1f else 1f - reveal.value }
                .background(MaterialTheme.colorScheme.surfaceContainer),
            contentAlignment = Alignment.Center,
        ) {
            MediaIcon(
                if (media.audio || media.kind == "album") Glyph.Music else Glyph.Preview,
                Modifier.size(28.dp),
                MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.25f),
            )
        }
        if (shown != null)
            Image(
                shown.asImageBitmap(),
                media.title,
                Modifier.fillMaxSize()
                    .graphicsLayer { alpha = reveal.value }
                    .then(
                        // Only the exact cover counts as ready; a stand-in is just painted.
                        if (bitmap != null)
                            Modifier.testTag("artwork-${media.key}").semantics { stateDescription = "Ready" }
                        else Modifier
                    ),
                contentScale = ContentScale.Crop,
                // Crop from the leading edge so captions composed near the left of
                // wide artwork survive inside a squarer container instead of being
                // centre-cropped off the left side.
                alignment = Alignment.CenterStart,
            )
    }
}

@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
@Composable
private fun MediaRow(
    vm: YtuiViewModel,
    media: MediaCard,
    tab: Int,
    thumbnails: Boolean,
    mobile: JSONObject,
    current: Boolean = false,
    playing: Boolean = false,
    shelf: Boolean = false,
    openMenu: (MediaCard, Boolean) -> Unit,
) {
    val square = tab in 2..3 || media.kind in setOf("album", "song", "station")
    // Feed videos sit inside a small side margin with rounded corners, the same treatment as
    // the cards in shelves, so a page never mixes square-cut and rounded artwork.
    val bleed = !square && !shelf
    val artShape = MediaTokens.medium
    val openMenu = { openMenu(media, square) }
    Column(
        Modifier.fillMaxWidth()
            .padding(horizontal = if (bleed) 12.dp else 0.dp)
            .testTag("media-card-${media.key}")
    ) {
        Box(
            Modifier.fillMaxWidth()
                .aspectRatio(if (square) 1f else 16f / 9f)
                .clip(artShape)
        ) {
            Artwork(
                media,
                Modifier.fillMaxSize()
                    .combinedClickable(
                        onLongClickLabel = "More options",
                        onLongClick = openMenu,
                        onClick = { vm.play(media) },
                    )
                    .semantics { contentDescription = "Play ${media.title}" },
                thumbnails,
                shape = artShape,
            )
            if (media.live || !square)
                MediaDurationBadge(media, mobile, Modifier.align(Alignment.BottomEnd))
            // Only the playing card composes its bars; an AnimatedVisibility per card kept a
            // transition alive in every cell of the grid.
            if (current) PlayingBars(playing, Modifier.align(Alignment.TopStart))
        }
        Row(
            Modifier.fillMaxWidth().padding(top = 10.dp),
            verticalAlignment = Alignment.Top,
        ) {
            if (bleed) {
                val showChannel = mobile.optBoolean("show_channel", true)
                ChannelAvatar(
                    if (showChannel) media.creator else "",
                    36.dp,
                    imageUrl = if (showChannel) media.channelThumbnail else "",
                    loadImage = thumbnails,
                    modifier =
                        Modifier.clip(CircleShape)
                            .opensChannel(media.creator, if (showChannel) media.channelId else "") {
                                vm.openChannel(it)
                            },
                )
                Spacer(Modifier.width(12.dp))
            }
            Column(
                Modifier.weight(1f)
                    .heightIn(min = 48.dp)
                    .combinedClickable(
                        onLongClickLabel = "More options",
                        onLongClick = openMenu,
                        onClick = { vm.play(media) },
                    )
                    .padding(top = 3.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Text(
                    media.title,
                    style = MaterialTheme.typography.titleSmall,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    color =
                        if (current)
                            mediaReadableText(
                                MaterialTheme.colorScheme.primary,
                                MaterialTheme.colorScheme.background,
                            )
                        else Color.Unspecified,
                )
                if (mobile.optBoolean("show_channel", true))
                    Text(
                        // Stations named after their genre ("Lofi" by "Lofi") say what kind
                        // of item they are instead of repeating the title.
                        media.creator.takeUnless { it.trim().equals(media.title.trim(), true) }.orEmpty().ifBlank {
                            if (media.collection) media.kind.replaceFirstChar(Char::titlecase)
                            else if (tab == 3)
                                media.value.optString("description").takeUnless { it.isBlank() || it == "null" }
                                    ?: "Live radio"
                            else "YouTube"
                        },
                        Modifier.opensChannel(media.creator, media.channelId) { vm.openChannel(it) },
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                CardInformation(media.value, mobile)
            }
            // Small shelf cards give the title the room; their menu opens on press and hold.
            if (!shelf) MediaCardActions(media, openMenu)
        }
    }
}

@Composable
private fun MediaCardActions(media: MediaCard, open: () -> Unit) {
    MediaIconButton(Glyph.More, "More options for ${media.title}", "media-menu-${media.key}", open)
}

@Composable
private fun MediaCardMenu(
    vm: YtuiViewModel,
    media: MediaCard,
    thumbnails: Boolean,
    square: Boolean,
    close: () -> Unit,
) {
    Panel("", close) {
        ActionSheetHeader(media, thumbnails, square)
        fun execute(action: () -> Unit) {
            close()
            action()
        }
        // The four things people do most, as big targets; the rest as rows below.
        Row(
            Modifier.fillMaxWidth().padding(bottom = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            QuickTile(
                Glyph.Play,
                if (media.collection) "Open" else "Play",
                Modifier.weight(1f),
                primary = true,
            ) {
                execute { vm.play(media) }
            }
            if (!media.collection && !media.live)
                QuickTile(Glyph.PlayNext, "Play next", Modifier.weight(1f)) {
                    execute { vm.playNext(media) }
                }
            QuickTile(Glyph.Queue, "Add to queue", Modifier.weight(1f)) {
                execute { vm.enqueue(media) }
            }
            if (!media.live) {
                var tileCentre by remember { mutableStateOf<androidx.compose.ui.geometry.Offset?>(null) }
                QuickTile(
                    Glyph.Download,
                    "Download",
                    Modifier.weight(1f).trackCentre { tileCentre = it },
                ) {
                    execute {
                        DownloadFlight.launch(media, tileCentre)
                        vm.play(media, download = true)
                    }
                }
            }
        }
        if (media.collection)
            ValueRow("Play all", glyph = Glyph.Queue, chevron = false) {
                execute { vm.playCollection(media) }
            }
        // A muted preview is rarely wanted, so it is a row rather than one of the four tiles.
        if (!media.collection && !media.live)
            ValueRow("Preview without sound", glyph = Glyph.Preview, chevron = false) {
                execute { vm.play(media, preview = true) }
            }
        if (media.channelId.isNotBlank() && media.kind !in setOf("channel", "artist"))
            ValueRow(
                "Go to channel",
                Modifier.testTag("media-channel-${media.key}"),
                glyph = Glyph.Account,
                subtitle = media.creator.ifBlank { "See all their videos" },
            ) {
                execute { vm.openChannel(media.channelId) }
            }
        ValueRow("Save to Library", glyph = Glyph.Save, chevron = false) {
            execute { vm.save(media) }
        }
        if (media.value.optString("tracks_url").let { it.isNotBlank() && it != "null" })
            ValueRow("Browse tracks", glyph = Glyph.Music) { execute { vm.tracks(media) } }
    }
}

/** Big square-ish action in an item's sheet: glyph over a short label. */
@Composable
private fun QuickTile(
    glyph: Glyph,
    label: String,
    modifier: Modifier = Modifier,
    primary: Boolean = false,
    action: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val source = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
    val ink = if (primary) colors.background else colors.onSurface
    CompositionLocalProvider(LocalContentColor provides ink) {
        Column(
            modifier
                .pressScale(source, 0.94f)
                .heightIn(min = 76.dp)
                .clip(MediaTokens.card)
                .background(if (primary) colors.onBackground else colors.onSurface.copy(alpha = 0.07f))
                .clickable(source, SoftIndication, role = Role.Button, onClick = action)
                .padding(horizontal = 6.dp, vertical = 12.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterVertically),
        ) {
            MediaIcon(glyph, Modifier.size(22.dp), ink)
            Text(
                label,
                style = MaterialTheme.typography.labelMedium,
                color = ink,
                textAlign = TextAlign.Center,
                maxLines = 2,
            )
        }
    }
}

/** Bottom-sheet header naming the item an action sheet acts on. */
@Composable
internal fun ActionSheetHeader(media: MediaCard, thumbnails: Boolean, square: Boolean = false) {
    Row(
        Modifier.fillMaxWidth().padding(start = 8.dp, end = 56.dp, top = 4.dp, bottom = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Artwork(
            media,
            Modifier.width(if (square) 64.dp else 112.dp).aspectRatio(if (square) 1f else 16f / 9f),
            thumbnails,
            shape = MediaTokens.medium,
        )
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Text(
                media.title,
                style = MaterialTheme.typography.titleMedium,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            if (media.creator.isNotBlank())
                Text(
                    media.creator,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
        }
    }
}

@Composable
private fun WideMediaRow(
    vm: YtuiViewModel,
    media: MediaCard,
    thumbnails: Boolean,
    mobile: JSONObject,
    openMenu: (MediaCard, Boolean) -> Unit,
) {
    Row(
        Modifier.fillMaxWidth()
            .testTag("media-card-${media.key}")
            .padding(horizontal = MediaTokens.textInset),
        verticalAlignment = Alignment.Top,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Box(Modifier.fillMaxWidth(0.25f).aspectRatio(16f / 9f)) {
            Artwork(media, Modifier.fillMaxSize().clickable { vm.play(media) }, thumbnails)
            MediaDurationBadge(media, mobile, Modifier.align(Alignment.BottomEnd))
        }
        Column(
            Modifier.weight(1f).clickable { vm.play(media) },
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                media.title,
                style = MaterialTheme.typography.titleMedium,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            CardInformation(media.value, mobile)
            if (mobile.optBoolean("show_channel", true))
                Row(
                    Modifier.opensChannel(media.creator, media.channelId) { vm.openChannel(it) },
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    ChannelAvatar(
                        media.creator,
                        24.dp,
                        imageUrl = media.channelThumbnail,
                        loadImage = thumbnails,
                    )
                    Text(
                        media.creator,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
        }
        MediaCardActions(media) { openMenu(media, false) }
    }
}

@Composable
private fun MediaDurationBadge(media: MediaCard, mobile: JSONObject, modifier: Modifier) {
    if (
        media.live || (!media.value.isNull("duration") && mobile.optBoolean("show_duration", true))
    )
        ArtworkBadge(
            media.live,
            if (media.live) "Live" else time(media.value.optLong("duration").toFloat()),
            modifier.testTag("duration-${media.key}"),
            pulse = false,
        )
}

/**
 * Playback options. `expanded` is the full options list (main sheet and fullscreen panel);
 * otherwise this is the tablet side pane's compact transport.
 */
@Composable
internal fun PlayerPane(
    vm: YtuiViewModel,
    expanded: Boolean,
    showSurface: Boolean = true,
    onQueue: () -> Unit,
    onSleep: () -> Unit = {},
    onDone: () -> Unit = {},
    onSettings: (() -> Unit)? = null,
    onOutput: (() -> Unit)? = null,
) {
    val player by vm.player.collectAsState()
    val config by vm.config.collectAsState()
    val mobile by vm.mobile.collectAsState()
    val tab by vm.tab.collectAsState()
    // In-sheet sub-pages (YouTube's quality/speed menus) keep one sheet and one Close.
    var page by remember { mutableStateOf("") }
    val optionsMotion = LocalMotion.current
    Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.TopCenter) {
        Column(
            Modifier.widthIn(max = if (player.video) 840.dp else 480.dp).fillMaxWidth(),
            verticalArrangement = Arrangement.spacedBy(if (expanded) 0.dp else 12.dp),
        ) {
            val preparing = player.preparing
            if (preparing != null) {
                Text(preparing.media.title, maxLines = 2, overflow = TextOverflow.Ellipsis)
                MediaProgressBar(null, Modifier.fillMaxWidth())
                return@Column
            }
            if (player.media == null) {
                EmptyMediaState(
                    Glyph.Music,
                    "Nothing playing yet",
                    "Choose a video, song or station to get started.",
                )
                return@Column
            }
            if (!expanded) {
                // Compact side panel (tablet): surface, title, seek and transport.
                if (player.video && showSurface) {
                    VideoSurface(
                        Modifier.fillMaxWidth().aspectRatio(16f / 9f).clip(MediaTokens.medium),
                        onSettings = onSettings,
                        onRecover =
                            if (player.value.optBoolean("recovered") && player.preparing == null)
                                ({ vm.command("pause") }) else null,
                    )
                } else if (showSurface && tab in 2..3) Visualizer(player, config)
                else if (showSurface)
                    Artwork(
                        MediaCard(player.media!!),
                        Modifier.fillMaxWidth().aspectRatio(16f / 9f),
                        config.optBoolean("thumbnails", true),
                    )
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(
                        player.title,
                        style = MaterialTheme.typography.titleLarge,
                        fontWeight = FontWeight.Bold,
                        maxLines = 3,
                        overflow = TextOverflow.Ellipsis,
                    )
                    (player.value.optString("station").ifBlank {
                            player.media?.optString("creator") ?: ""
                        })
                        .takeIf { it.isNotBlank() }
                        ?.let {
                            Text(
                                it,
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                }
                if (player.video) player.media?.let(::playerInformation)?.takeIf { it.isNotBlank() }?.let {
                    Text(
                        it,
                        Modifier.testTag("player-information"),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                player.value
                    .optString("error")
                    .takeUnless { it.isBlank() || it == "null" }
                    ?.let { Text(it, color = MaterialTheme.colorScheme.error) }
            }
            if (player.preview) {
                Text(
                    "Muted preview",
                    Modifier.padding(vertical = 8.dp),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Action("Play full") { vm.promotePreview() }
                    SecondaryAction("End preview") { vm.command("preview_end") }
                }
                return@Column
            }
            if (!expanded) {
                if (player.duration > 0 && player.media?.optBoolean("live") != true) {
                    var seek by
                        remember(player.media?.optString("url")) { mutableStateOf<Float?>(null) }
                    Column {
                        MediaSeekSlider(
                            (seek ?: player.position).coerceIn(0f, player.duration),
                            { seek = it },
                            valueRange = 0f..player.duration,
                            onValueChangeFinished = {
                                seek?.let { vm.command("seek", "seconds" to it) }
                                seek = null
                            },
                            modifier = Modifier.fillMaxWidth().testTag("seek-slider"),
                            marks = player.sponsorMarks(),
                        )
                        Row(Modifier.fillMaxWidth()) {
                            Text(
                                time(seek ?: player.position),
                                style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                            Spacer(Modifier.weight(1f))
                            Text(
                                time(player.duration),
                                style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                }
                Row(
                    Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceEvenly,
                ) {
                    TransportButton(Glyph.Previous, "Previous track") { vm.command("previous") }
                    val seekSeconds = mobile.optInt("seek_seconds", 10)
                    SeekJump(seekSeconds, forward = false, "Back $seekSeconds seconds") { vm.command("back") }
                    PlayDisc(player.playing, Modifier.testTag("player-pause"), size = 64.dp) {
                        vm.command("pause")
                    }
                    SeekJump(seekSeconds, forward = true, "Forward $seekSeconds seconds") { vm.command("forward") }
                    TransportButton(Glyph.Next, "Next track") { vm.command("next") }
                }
                Row(
                    Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceEvenly,
                ) {
                    MediaModeButton(
                        Glyph.Shuffle,
                        "Shuffle queue",
                        "pane-shuffle",
                        player.value.optBoolean("shuffle"),
                    ) {
                        vm.command("shuffle")
                    }
                    MediaModeButton(
                        Glyph.Repeat,
                        "Repeat mode",
                        "pane-repeat",
                        player.value.optBoolean("repeat_one"),
                    ) {
                        vm.command("repeat")
                    }
                    // Stop lives in Playback options only; pausing is enough here.
                    MediaIconButton(Glyph.UpNext, "Playback queue") { onQueue() }
                }
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center) {
                    SecondaryAction("Open full player", glyph = Glyph.Expand) {
                        vm.playerOpen.value = true
                    }
                }
                return@Column
            }
            @Composable
            fun SubPage(title: String, content: @Composable () -> Unit) {
                SheetAction(
                    text = { Text(title, fontWeight = FontWeight.SemiBold) },
                    leadingIcon = { MediaIcon(Glyph.Back) },
                    onClick = { page = "" },
                    modifier =
                        Modifier.semantics { contentDescription = "Back to playback options" },
                )
                HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                content()
            }
            @Composable
            fun Option(label: String, chosen: Boolean, choose: () -> Unit) {
                SheetAction(
                    text = { Text(label) },
                    leadingIcon = { ChoiceMark(chosen) },
                    onClick = choose,
                    modifier = Modifier.semantics { selected = chosen },
                )
            }
            val speed = player.value.optDouble("speed", 1.0)
            fun speedLabel(value: Double) =
                if (value == 1.0) "Normal"
                else "${if (value % 1.0 == 0.0) value.toInt().toString() else value.toString()}×"
            val quality = player.value.optString("quality", "1080")
            fun qualityLabel(value: String) =
                when (value) {
                    "auto" -> "Auto"
                    "best" -> "Best"
                    else -> "${value}p"
                }
            val qualityAvailable =
                player.video &&
                    !player.value.optBoolean("offline") &&
                    player.media?.optBoolean("live") != true
            // Sub-pages slide in from the side and back, like a navigation stack, and the
            // sheet's height eases to fit instead of jumping.
            AnimatedContent(
                page,
                transitionSpec = {
                    if (!optionsMotion.animate) EnterTransition.None togetherWith ExitTransition.None
                    else {
                        val forward = targetState.isNotEmpty()
                        (fadeIn(optionsMotion.fade(180)) +
                            slideInHorizontally(optionsMotion.enter(300)) { if (forward) it / 3 else -it / 3 }) togetherWith
                            (fadeOut(optionsMotion.fade(120)) +
                                slideOutHorizontally(optionsMotion.exit(220)) { if (forward) -it / 4 else it / 4 }) using
                            SizeTransform(clip = false) { _, _ -> optionsMotion.spring() }
                    }
                },
                label = "player-options-page",
            ) { shownPage ->
            Column {
            when (shownPage) {
                "quality" ->
                    SubPage("Video quality") {
                        val available = player.value.optJSONArray("video_qualities")
                        val qualities = (0 until (available?.length() ?: 0))
                            .mapNotNull { available?.optInt(it)?.takeIf { height -> height > 0 } }
                            .distinct().sortedDescending().map { it.toString() }
                        // Auto is opt-in, never the default.
                        for (value in listOf("auto", "best") + qualities) Option(
                            qualityLabel(value),
                            value == quality,
                        ) {
                            vm.command("quality", "value" to value)
                            page = ""
                        }
                    }
                "captions" ->
                    SubPage("Captions") {
                        CaptionsOptions(player.value) { op, fields -> vm.command(op, *fields) }
                    }
                "audio" ->
                    SubPage("Audio") {
                        AudioTrackOptions(player.value) { op, fields -> vm.command(op, *fields) }
                    }
                else -> {
                    Row(
                        Modifier.fillMaxWidth().heightIn(min = 56.dp).padding(horizontal = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(16.dp),
                    ) {
                        MediaIcon(Glyph.Volume, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                        val volume =
                            player.value.optDouble("volume", 0.7).toFloat().coerceIn(0f, 1f)
                        // Same slim line and round dot as the seek bar.
                        MediaSeekSlider(
                            volume,
                            { vm.command("volume", "value" to it) },
                            valueRange = 0f..1f,
                            onValueChangeFinished = {},
                            modifier =
                                Modifier.weight(1f).testTag("volume-slider").semantics {
                                    contentDescription = "Volume"
                                },
                        )
                        Text(
                            "${(volume * 100).roundToInt()}%",
                            Modifier.widthIn(min = 40.dp),
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    // The common speeds are one tap away; the full list stays a row below.
                    SpeedChips(speed, ::speedLabel) { vm.command("speed", "value" to it) }
                    if (qualityAvailable)
                        ValueRow(
                            "Video quality",
                            value = qualityLabel(quality),
                            glyph = Glyph.Settings,
                        ) {
                            page = "quality"
                        }
                    ValueRow(
                        "Captions",
                        Modifier.testTag("options-captions"),
                        value = captionSummary(player.value),
                        glyph = Glyph.Captions,
                    ) {
                        page = "captions"
                    }
                    if (audioChoices(player.value))
                        ValueRow(
                            "Audio",
                            Modifier.testTag("options-audio"),
                            value = audioSummary(player.value),
                            glyph = Glyph.Volume,
                        ) {
                            page = "audio"
                        }
                    ValueRow("Playback queue", glyph = Glyph.UpNext) { onQueue() }
                    // Audio output moved here from the player's tray.
                    if (onOutput != null)
                        player.output?.let { output ->
                            ValueRow(
                                "Audio output",
                                value = output.name,
                                glyph = if (output.bluetooth) Glyph.Bluetooth else Glyph.Headphones,
                                modifier = Modifier.testTag("player-output"),
                            ) {
                                onOutput()
                            }
                        }
                    val sleepLeft = rememberSleepLeft(player)
                    ValueRow(
                        "Sleep timer",
                        value = if (sleepLeft > 0) sleepLabel(sleepLeft) else "Off",
                        glyph = Glyph.Sleep,
                    ) {
                        onSleep()
                    }
                    val segments = player.value.optJSONArray("segments")?.objects() ?: emptyList()
                    if (segments.isNotEmpty() && player.duration > 0) {
                        SectionHeader("Marked segments")
                        val accent = MaterialTheme.colorScheme.primary
                        val muted = MaterialTheme.colorScheme.outlineVariant
                        Canvas(
                            Modifier.fillMaxWidth()
                                .padding(horizontal = 8.dp, vertical = 4.dp)
                                .height(6.dp)
                                .clip(MediaTokens.pill)
                                .testTag("sponsor-marks")
                        ) {
                            drawRect(muted)
                            segments.forEach { mark ->
                                val start =
                                    mark.optDouble("start").toFloat().coerceIn(0f, player.duration)
                                val end =
                                    mark.optDouble("end").toFloat().coerceIn(start, player.duration)
                                drawRect(
                                    accent,
                                    Offset(start / player.duration * size.width, 0f),
                                    androidx.compose.ui.geometry.Size(
                                        (end - start) / player.duration * size.width,
                                        size.height,
                                    ),
                                )
                            }
                        }
                        segments.forEach { mark ->
                            ValueRow(
                                mark.optString("category").replaceFirstChar(Char::titlecase),
                                value = time(mark.optDouble("start").toFloat()),
                                glyph = Glyph.Play,
                                subtitle = "Watch without skipping",
                                chevron = false,
                            ) {
                                vm.command(
                                    "sponsor_watch",
                                    "start" to mark.optDouble("start"),
                                    "end" to mark.optDouble("end"),
                                )
                            }
                        }
                        ValueRow("Replay skipped segment", glyph = Glyph.Repeat, chevron = false) {
                            vm.command("sponsor_replay")
                        }
                    }
                    HorizontalDivider(
                        Modifier.padding(vertical = 4.dp),
                        color = MaterialTheme.colorScheme.outlineVariant,
                    )
                    ValueRow("Stop playback", glyph = Glyph.Stop, chevron = false) {
                        vm.command("stop")
                        onDone()
                    }
                }
            }
            }
            }
        }
    }
}

/** Width/height of the music artwork area for a player style: square except the vinyl deck. */
internal fun visualizerRatio(style: String): Float = if (style.startsWith("vinyl")) 1.35f else 1f

/**
 * The style Android shows. The "-fullscreen" variants stay valid in the shared settings (the
 * desktop app uses them), but Android no longer offers or acts on them: each one shows as its
 * normal counterpart.
 */
internal fun androidPlayerStyle(config: JSONObject): String =
    config.optString("music_player_style", "classic").removeSuffix("-fullscreen")

@Composable
internal fun Visualizer(player: PlayerState, config: JSONObject, modifier: Modifier = Modifier) {
    val style = androidPlayerStyle(config)
    val reduced = config.optBoolean("reduced_motion")
    val lifecycleState by LocalLifecycleOwner.current.lifecycle.currentStateFlow.collectAsState()
    val visible = lifecycleState.isAtLeast(Lifecycle.State.STARTED)
    var phase by remember { mutableFloatStateOf(0f) }
    // The liquid keeps its own clock. It is read only while drawing, so the animation never
    // recomposes this screen.
    val levels = remember(style) { LiquidLevels() }
    var liquidTime by remember(style) { mutableFloatStateOf(0f) }
    LaunchedEffect(player.playing, reduced, style, visible) {
        if (!player.playing && !reduced && visible && (style.startsWith("vu") || style.startsWith("orbit"))) {
            // Paused: let the bars, beat thump and shake ease out instead of freezing mid-beat.
            val silence = IntArray(SPECTRUM_BANDS)
            var previous = 0L
            val until = System.nanoTime() + 900_000_000L
            while (System.nanoTime() < until) withInfiniteAnimationFrameNanos { now ->
                val elapsed = if (previous == 0L) 0f else (now - previous) / 1_000_000_000f
                previous = now
                levels.step(silence, elapsed)
                liquidTime += elapsed
            }
        }
        if (player.playing && !reduced && style != "classic" && visible) {
            var previous = 0L
            while (true) withInfiniteAnimationFrameNanos { now ->
                val elapsed = if (previous == 0L) 0f else (now - previous) / 1_000_000_000f
                if (previous != 0L) phase = (phase + elapsed * 90) % 360
                previous = now
                if (style.startsWith("vu") || style.startsWith("orbit")) {
                    levels.step(AudioTap.spectrum, elapsed)
                    liquidTime += elapsed
                }
            }
        }
    }
    DisposableEffect(player.playing, style, reduced, visible) {
        AudioTap.enabled =
            visible &&
                player.playing &&
                !reduced &&
                (style.startsWith("vu") || style.startsWith("orbit"))
        onDispose { AudioTap.enabled = false }
    }
    val accent = MaterialTheme.colorScheme.primary
    val ink = MaterialTheme.colorScheme.onBackground
    val media = player.media?.let(::MediaCard) ?: return
    // Bars and strings take the cover's own dominant colours.
    val swatches = rememberArtworkSwatches(media, config.optBoolean("thumbnails", true))
    val cornerRadius = if (style == "classic") 20.dp else 4.dp
    BoxWithConstraints(
        modifier
            .fillMaxWidth()
            .aspectRatio(visualizerRatio(style))
            .then(
                if (style == "classic")
                    Modifier.shadow(
                        28.dp,
                        RoundedCornerShape(cornerRadius),
                        ambientColor = Color.Black,
                        spotColor = Color.Black,
                    )
                else Modifier
            )
            .clip(RoundedCornerShape(cornerRadius))
            .testTag("music-visualizer"),
        contentAlignment = Alignment.CenterStart,
    ) {
        if (style == "classic")
            Artwork(
                media,
                Modifier.fillMaxSize().align(Alignment.Center).sharedCover(media.key, LocalPlayerExpanded.current),
                config.optBoolean("thumbnails", true),
                shape = RoundedCornerShape(cornerRadius),
            )
        else if (style.startsWith("vinyl")) {
            Canvas(Modifier.fillMaxSize()) {
                val radius = min(size.height * 0.48f, size.width * 0.32f)
                val center = Offset(size.width * 0.65f, size.height / 2)
                drawCircle(Color(0xff131516), radius, center)
                for (ring in 1..12) drawCircle(
                    ink.copy(alpha = 0.13f),
                    radius * ring / 12,
                    center,
                    style = Stroke(1.5f),
                )
                rotate(phase, center) {
                    drawLine(
                        ink.copy(alpha = 0.25f),
                        center,
                        center + Offset(radius * 0.85f, 0f),
                        5f,
                    )
                    drawCircle(accent, radius * 0.24f, center)
                    drawCircle(Color.Black, radius * 0.025f, center)
                }
            }
            val label = maxWidth * 0.16f
            Artwork(
                media,
                Modifier.align(Alignment.TopStart)
                    .offset(x = maxWidth * 0.65f - label / 2, y = maxHeight / 2 - label / 2)
                    .size(label)
                    .graphicsLayer { rotationZ = phase }
                    .clip(CircleShape)
                    .testTag("vinyl-record-label"),
                config.optBoolean("thumbnails", true),
            )
            Artwork(
                media,
                Modifier.fillMaxHeight(0.94f).aspectRatio(1f),
                config.optBoolean("thumbnails", true),
            )
        } else if (style.startsWith("vu")) {
            // The liquid fills the tank behind a large cover; spectrum bars rise inside it.
            Box(Modifier.fillMaxSize()) {
                LiquidTank(levels, { liquidTime }, accent, ink, Modifier.fillMaxSize(), swatches)
                Artwork(
                    media,
                    Modifier.fillMaxWidth(0.6f)
                        .aspectRatio(1f)
                        .align(Alignment.Center)
                        .beat(levels) { liquidTime }
                        .shadow(18.dp, MediaTokens.card, ambientColor = Color.Black, spotColor = Color.Black)
                        .clip(MediaTokens.card),
                    config.optBoolean("thumbnails", true),
                    shape = MediaTokens.card,
                )
            }
        } else {
            // A glow, a ring of spectrum bars and two liquid layers around a large round cover
            // that swells slightly on the beat.
            LiquidOrb(levels, { liquidTime }, accent, ink, Modifier.fillMaxSize(), swatches)
            Artwork(
                media,
                Modifier.fillMaxWidth(0.56f)
                    .aspectRatio(1f)
                    .align(Alignment.Center)
                    .beat(levels) { liquidTime }
                    .shadow(16.dp, CircleShape, ambientColor = Color.Black, spotColor = Color.Black)
                    .clip(CircleShape),
                config.optBoolean("thumbnails", true),
                shape = CircleShape,
            )
        }
    }
}

@Composable
private fun SettingsPanel(
    vm: YtuiViewModel,
    dark: Boolean,
    onAccount: () -> Unit,
    onSetup: () -> Unit,
    /** Opens another sheet by name: "history", "queue", "sleep" or "about". */
    onOpen: (String) -> Unit = {},
    close: () -> Unit,
) {
    val account by vm.account.collectAsState()
    var category by rememberSaveable { mutableStateOf("") }
    val mobile by vm.mobile.collectAsState()
    val config by vm.config.collectAsState()
    val themes by vm.themes.collectAsState()
    val appliedPalette by vm.palette.collectAsState()
    val selectedTheme =
        when (val selected = config.optString("theme", "system")) {
            "system",
            "custom" -> selected
            else -> appliedPalette.optString("name").ifBlank { selected }
        }
    var themeMenu by remember { mutableStateOf(false) }
    var proxy by
        remember(config.optString("proxy")) {
            mutableStateOf(config.optString("proxy").takeUnless { it == "null" } ?: "")
        }
    // Wide windows show the sections beside their content instead of one page at a time.
    val twoPane = LocalConfiguration.current.screenWidthDp >= 960
    val shown = if (twoPane && category.isBlank()) "Appearance" else category
    Panel(
        if (twoPane) "You" else category.ifBlank { "You" },
        close,
        page = true,
        scroll = !twoPane,
        headerBack = if (!twoPane && category.isNotBlank()) ({ category = "" }) else null,
        headerBackTag = "settings-back",
    ) {
        BackHandler(category.isNotBlank() && !twoPane) { category = "" }
        @Composable
        fun RootList() {
            // Who you are first: the account card opens sign-in or the session details.
            val connected = account.optBoolean("connected")
            ValueRow(
                if (connected) "Signed in to YouTube" else "Sign in",
                Modifier.testTag("settings-Account"),
                glyph = Glyph.Account,
                subtitle =
                    if (connected) "Your subscriptions, playlists and likes are available"
                    else "Get your subscriptions, playlists and likes",
            ) {
                onAccount()
            }
            SettingGroup("Your activity") {
                ValueRow("History", Modifier.testTag("settings-History"), glyph = Glyph.History, subtitle = "What you played recently") {
                    onOpen("history")
                }
                ValueRow("Playback queue", Modifier.testTag("settings-Queue"), glyph = Glyph.UpNext, subtitle = "What plays next") {
                    onOpen("queue")
                }
                ValueRow("Sleep timer", Modifier.testTag("settings-Sleep"), glyph = Glyph.Sleep, subtitle = "Stop playback after a while") {
                    onOpen("sleep")
                }
            }
            for ((group, rows) in
                listOf(
                    "Look and feel" to
                        listOf(
                            Triple("Appearance", Glyph.Palette, "Theme, player style, motion"),
                            Triple("Gestures", Glyph.Drag, "Swipes and taps in lists and the player"),
                            Triple("Card details", Glyph.Info, "Channel, views, dates and more on each card"),
                        ),
                    "Listening" to
                        listOf(
                            Triple("Playback", Glyph.Play, "Quality, SponsorBlock, equalizer"),
                            Triple("Offline & storage", Glyph.Download, "Downloads, audio-only, folders"),
                        ),
                    "This device" to
                        listOf(
                            Triple("Network", Glyph.Link, "Proxy"),
                            Triple("Privacy", Glyph.History, "History and resume"),
                            Triple("Advanced", Glyph.Settings, "Animation tuning, cache, diagnostics, Movies"),
                            Triple("Setup guide", Glyph.Refresh, "Replay the welcome and setup steps"),
                            Triple("About", Glyph.Info, "Version, credits and help"),
                        ),
                )) {
                SettingGroup(group) {
                    for ((label, icon, detail) in rows)
                        ValueRow(
                            label,
                            glyph = icon,
                            subtitle = detail,
                            modifier =
                                Modifier.testTag("settings-$label").then(
                                    if (twoPane && shown == label)
                                        Modifier.clip(MediaTokens.medium)
                                            .background(MaterialTheme.colorScheme.onSurface.copy(alpha = 0.08f))
                                    else Modifier
                                ),
                        ) {
                            when (label) {
                                "Setup guide" -> onSetup()
                                "About" -> onOpen("about")
                                else -> category = label
                            }
                        }
                }
            }
            Text(
                "Vidcue ${BuildConfig.VERSION_NAME}",
                Modifier.fillMaxWidth().padding(vertical = 20.dp),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
            )
        }
        @Composable
        fun CategoryContent() {
        if (shown == "Appearance")
            SettingGroup("Appearance", "Colour, artwork and motion", showTitle = false) {
                Box {
                    val palettes by vm.palettes.collectAsState()
                    LaunchedEffect(Unit) { vm.loadPalettes() }
                    val current = config.optString("theme", "system")
                    SheetAction(
                        modifier = Modifier.testTag("theme-selector"),
                        text = {
                            Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                Text("Theme", style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.Medium)
                                Text(
                                    if (current == "custom") "Imported palette" else themeLabel(current),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        },
                        trailing = {
                            PaletteSwatch(
                                if (current == "custom") appliedPalette else palettes.optJSONObject(current)
                            )
                        },
                        onClick = { themeMenu = true },
                    )
                    if (themeMenu)
                        MediaDialog(
                            { themeMenu = false },
                            "Theme",
                            actions = { QuietAction("Cancel") { themeMenu = false } },
                        ) {
                            Column(
                                Modifier.heightIn(max = LocalConfiguration.current.screenHeightDp.dp * 0.6f)
                                    .verticalScroll(rememberScrollState())
                            ) {
                                themes.forEach { name ->
                                    Row(
                                        Modifier.fillMaxWidth()
                                            .heightIn(min = 52.dp)
                                            .clip(MediaTokens.medium)
                                            .testTag("theme-$name")
                                            .selectable(
                                                selected = selectedTheme == name,
                                                role = Role.RadioButton,
                                            ) {
                                                themeMenu = false
                                                vm.updateConfig(dark) { it.put("theme", name) }
                                            }
                                            .padding(horizontal = 8.dp, vertical = 4.dp),
                                        verticalAlignment = Alignment.CenterVertically,
                                        horizontalArrangement = Arrangement.spacedBy(14.dp),
                                    ) {
                                        ChoiceMark(selectedTheme == name)
                                        Text(
                                            themeLabel(name),
                                            Modifier.weight(1f),
                                            style = MaterialTheme.typography.bodyLarge,
                                        )
                                        PaletteSwatch(palettes.optJSONObject(name))
                                    }
                                }
                            }
                        }
                }
                ValueRow(
                    "Import a palette",
                    subtitle = "Caelestia, Omarchy or Vidcue palette files",
                    chevron = false,
                ) {
                    vm.effect("pick", "kind" to "theme")
                }
                SettingToggle(
                    "Thumbnails",
                    config.optBoolean("thumbnails", true),
                    "Cache artwork at the size you need",
                ) { value ->
                    vm.updateConfig(dark) { it.put("thumbnails", value) }
                }
                SettingToggle(
                    "Reduce motion",
                    config.optBoolean("reduced_motion"),
                    "Keep visualisers still and turn off all animations",
                ) { value ->
                    vm.updateConfig(dark) { it.put("reduced_motion", value) }
                }
                Choice(
                    "Music player",
                    androidPlayerStyle(config),
                    listOf("classic", "vinyl", "vu", "orbit"),
                    label = { PLAYER_STYLE_LABELS[it] ?: choiceLabel(it) },
                ) { value ->
                    vm.updateConfig(dark) { it.put("music_player_style", value) }
                }
            }
        if (shown == "Playback")
            SettingGroup("Playback", showTitle = false) {
                // One setting backs both playback and offline saves.
                Choice(
                    "Maximum video quality",
                    config.optJSONObject("download")?.optString("quality", "1080") ?: "1080",
                    listOf("360", "480", "720", "1080", "best"),
                    subtitle = "Also limits videos you download",
                ) { value ->
                    vm.updateConfig(dark) { it.getJSONObject("download").put("quality", value) }
                }
                Choice(
                    "SponsorBlock",
                    config.optString("playback_sponsorblock", "skip"),
                    listOf("skip", "mark", "off"),
                ) { value ->
                    vm.updateConfig(dark) { it.put("playback_sponsorblock", value) }
                }
                EqualizerSettings(vm, dark, config)
            }
        if (shown == "Privacy")
            SettingGroup("Privacy", showTitle = false) {
                SettingToggle(
                    "History & resume",
                    config.optBoolean("playback_history", true),
                    "Remember playback and recent searches on this device",
                ) { value ->
                    vm.updateConfig(dark) { it.put("playback_history", value) }
                }
            }
        if (shown == "Advanced") {
            SettingGroup("Animation tuning") {
                Choice(
                    "Animation style",
                    mobile.optString("animation_style", "smooth"),
                    listOf("smooth", "snappy", "bouncy", "gentle"),
                    subtitle =
                        when (mobile.optString("animation_style", "smooth")) {
                            "snappy" -> "Short, crisp transitions"
                            "bouncy" -> "Springy, with a little overshoot"
                            "gentle" -> "Longer, softer transitions"
                            else -> "Fluid ease-out transitions (default)"
                        },
                ) {
                    vm.updateMobile("animation_style", it)
                }
                Choice(
                    "Animation speed",
                    animationSpeedLabel(mobile.optInt("animation_speed", 100)),
                    ANIMATION_SPEEDS.map { it.second },
                ) { label ->
                    ANIMATION_SPEEDS.firstOrNull { it.second == label }?.let {
                        vm.updateMobile("animation_speed", it.first)
                    }
                }
            }
            SettingGroup("Data and cache") {
                SettingToggle(
                    "Preload on mobile data",
                    mobile.optBoolean("preload_mobile", MobileSettings.BOOLEANS["preload_mobile"] == true),
                    "Prepare the next items on cellular too; uses more data",
                ) {
                    vm.updateMobile("preload_mobile", it)
                }
                Choice(
                    "Streaming cache (MiB)",
                    mobile.optInt("stream_cache_mb", 128).toString(),
                    listOf("32", "64", "128", "256", "512"),
                    subtitle = "Applies after restarting the app. Separate from your downloads",
                ) {
                    vm.updateMobile("stream_cache_mb", it.toInt())
                }
            }
            SettingGroup("Experiments") {
                SettingToggle(
                    "Movies (alpha)",
                    mobile.optBoolean("show_movies", false),
                    "Adds a Movies tab with openly licensed films and test streams",
                ) {
                    vm.updateMobile("show_movies", it)
                }
            }
        }
        if (shown == "Advanced") {
            val player by vm.player.collectAsState()
            val report = remember(player) { StartupTimings.report() }
            val context = androidx.compose.ui.platform.LocalContext.current
            SettingGroup(
                "Playback startup",
                "Local measurements, including extraction and visible video output",
            ) {
                Text(
                    report,
                    Modifier.testTag("startup-timings"),
                    style = MaterialTheme.typography.bodySmall,
                )
                SecondaryAction("Copy timing report", Modifier.testTag("copy-startup-timings")) {
                    val clipboard =
                        context.getSystemService(android.content.Context.CLIPBOARD_SERVICE)
                            as android.content.ClipboardManager
                    clipboard.setPrimaryClip(
                        android.content.ClipData.newPlainText(
                            "Vidcue startup timings",
                            BuildConfig.VERSION_NAME +
                                "\n" +
                                BuildConfig.SOURCE_REVISION +
                                "\n" +
                                report,
                        )
                    )
                }
            }
        }
        if (shown == "Network")
            SettingGroup(
                "Network",
                "Your Android VPN works normally. Changes apply to new connections.",
                showTitle = false,
            ) {
                MediaTextField(
                    proxy,
                    { proxy = it },
                    "HTTP proxy (optional)",
                    Modifier.testTag("proxy-field"),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
                )
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                    Action("Apply network settings") {
                        vm.updateConfig(dark) { it.put("proxy", proxy.trim().ifBlank { null }) }
                    }
                }
            }
        if (shown == "Appearance")
            SettingGroup("Motion") {
                val reducedMotion = config.optBoolean("reduced_motion")
                if (reducedMotion)
                    Text(
                        "Reduce motion is on, so animations are off. These choices apply when it is off.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                SettingToggle(
                    "Animated cards",
                    mobile.optBoolean("animate_cards", true),
                    "Cards press in when touched and ease in as a list loads",
                ) {
                    vm.updateMobile("animate_cards", it)
                }
                SettingToggle(
                    "Haptic feedback",
                    mobile.optBoolean("haptics", true),
                    "A light tick when a gesture takes effect",
                ) {
                    vm.updateMobile("haptics", it)
                }
            }
        if (shown == "Gestures") {
            val gesturesOn = mobile.optBoolean("gestures", true)
            SettingGroup("Gestures", showTitle = false) {
                SettingToggle("Swipe gestures", gesturesOn, "Turns every gesture below on or off") {
                    vm.updateMobile("gestures", it)
                }
                val context = androidx.compose.ui.platform.LocalContext.current
                ValueRow(
                    "Show gesture tips again",
                    subtitle = "The mini player shows how to swipe it next time it appears",
                    chevron = false,
                ) {
                    GestureTips.reset(context)
                    vm.confirm("tips")
                }
            }
            if (gesturesOn)
                for ((group, keys) in GESTURE_GROUPS)
                    SettingGroup(group) {
                        for ((key, title, detail) in GESTURE_SETTINGS.filter { it.first in keys })
                            SettingToggle(title, mobile.optBoolean(key, MobileSettings.BOOLEANS[key] == true), detail) {
                                vm.updateMobile(key, it)
                            }
                        if ("gesture_hold_speed" in keys && mobile.optBoolean("gesture_hold_speed", true))
                            Choice(
                                "Hold-to-speed rate",
                                "${mobile.optInt("hold_speed", 200) / 100f}×".replace(".0×", "×"),
                                listOf("1.25×", "1.5×", "2×", "3×"),
                            ) {
                                val percent =
                                    (it.removeSuffix("×").toFloat() * 100).roundToInt()
                                vm.updateMobile("hold_speed", percent)
                            }
                    }
        }
        if (shown == "Playback")
            SettingGroup("Fast playback") {
                for ((key, label) in
                    listOf(
                        "preload_next" to "Preload visible and next items",
                        "autoplay_related" to "Related media autoplay",
                        "resume_on_play" to "Resume videos where you left off",
                        "prefer_offline" to "Play downloads when available",
                        "background_playback" to "Background playback",
                    )) {
                    SettingToggle(
                        label,
                        mobile.optBoolean(key, MobileSettings.BOOLEANS[key] == true),
                    ) {
                        vm.updateMobile(key, it)
                    }
                }
                Choice(
                    "Colour grading",
                    mobile.optString("color_grade", "off"),
                    ColorGradeMath.PRESETS,
                ) {
                    vm.updateMobile("color_grade", it)
                }
                SettingToggle(
                    "Auto colour grade",
                    mobile.optBoolean("auto_color_grade", false),
                    "Analyses the playing video and adds a slight fitting grade",
                ) {
                    vm.updateMobile("auto_color_grade", it)
                }
                Choice(
                    "Seek interval",
                    mobile.optInt("seek_seconds", 10).toString(),
                    listOf("5", "10", "15", "30"),
                ) {
                    vm.updateMobile("seek_seconds", it.toInt())
                }
            }
        if (shown == "Offline & storage")
            SettingGroup("Downloads", showTitle = false) {
                SettingToggle(
                    "Audio-only downloads",
                    config.optJSONObject("download")?.optBoolean("audio") == true,
                    "Download YouTube Music without video. YouTube videos always keep video, " +
                        "up to the maximum quality set in Playback.",
                ) { value ->
                    vm.updateConfig(dark) { it.getJSONObject("download").put("audio", value) }
                }
                Choice(
                    "Simultaneous downloads",
                    mobile.optInt("download_parallel", 3).toString(),
                    listOf("1", "2", "3", "4"),
                ) {
                    vm.updateMobile("download_parallel", it.toInt())
                }
                SettingToggle(
                    "Download on Wi-Fi only",
                    mobile.optBoolean("download_unmetered", false),
                    "Downloads wait for Wi-Fi or another unmetered network.",
                ) {
                    vm.updateMobile("download_unmetered", it)
                }
                SectionHeader("Folders")
                SettingToggle(
                    "Copy finished downloads to folders",
                    mobile.optBoolean("auto_save_folders", true),
                    "Real files in your Music or Video folder; playlist downloads keep their own subfolder.",
                ) {
                    vm.updateMobile("auto_save_folders", it)
                }
                SettingToggle(
                    "Convert audio downloads to MP3",
                    mobile.optBoolean("auto_mp3", false),
                    "Saves finished audio as MP3 in your Music folder, and converts audio you already downloaded. Needs a Music folder.",
                ) {
                    vm.updateMobile("auto_mp3", it)
                }
                for ((label, audio, key) in
                    listOf(
                        Triple("Music folder", true, "music_folder"),
                        Triple("Video folder", false, "video_folder"),
                    )) {
                    ValueRow(
                        label,
                        subtitle = mobile.optString(key, "App storage").ifBlank { "App storage" },
                    ) {
                        vm.effect("folder_pick", "audio" to audio)
                    }
                }
                ValueRow("Retry folder copies", chevron = false) {
                    vm.effect("folder_retry")
                }
            }
        if (shown == "Card details")
            SettingGroup(
                "Visible information",
                subtitle = "Choose details for Home, search and feed cards. The player always shows available video information.",
            ) {
                for ((key, label) in
                    listOf(
                        "show_channel" to "Channel or artist",
                        "show_views" to "View count",
                        "show_upload_date" to "Upload date",
                        "show_duration" to "Video duration",
                        "show_likes" to "Likes",
                        "show_description" to "Description",
                        "show_section" to "Feed section",
                    )) {
                    SettingToggle(
                        label,
                        mobile.optBoolean(key, MobileSettings.BOOLEANS[key] == true),
                    ) {
                        vm.updateMobile(key, it)
                    }
                }
            }
        }
        if (twoPane)
            Row(Modifier.fillMaxSize(), horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                Column(
                    Modifier.width(360.dp).fillMaxHeight().verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    RootList()
                }
                Column(
                    Modifier.weight(1f).fillMaxHeight().verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Text(shown, style = MaterialTheme.typography.headlineSmall)
                    CategoryContent()
                }
            }
        else {
            if (category.isBlank()) RootList()
            CategoryContent()
        }
    }
}

private val ANIMATION_SPEEDS =
    listOf(50 to "Slowest", 75 to "Slow", 100 to "Normal", 150 to "Fast", 200 to "Fastest")

private fun animationSpeedLabel(percent: Int) =
    ANIMATION_SPEEDS.minByOrNull { abs(it.first - percent) }?.second ?: "Normal"

/** Gesture pages: where each gesture acts. */
private val GESTURE_GROUPS =
    listOf(
        "Pages" to setOf("gesture_swipe_tabs"),
        "Players" to setOf("gesture_swipe_dismiss", "gesture_mini_player", "gesture_swipe_tracks"),
        "Video" to
            setOf("gesture_double_tap_seek", "gesture_hold_speed", "gesture_video_adjust", "gesture_pinch_zoom"),
    )

private val GESTURE_SETTINGS =
    listOf(
        Triple(
            "gesture_swipe_tabs",
            "Swipe between tabs",
            "Swipe a page left or right to open the next tab",
        ),
        Triple(
            "gesture_swipe_dismiss",
            "Swipe down to minimize",
            "Pull the player down to shrink it to the mini player",
        ),
        Triple(
            "gesture_mini_player",
            "Mini player swipes",
            "Sideways: next or previous · Up: open · Down: stop",
        ),
        Triple(
            "gesture_swipe_tracks",
            "Swipe artwork to change track",
            "Swipe the music artwork left for next, right for previous",
        ),
        Triple(
            "gesture_double_tap_seek",
            "Double-tap to seek",
            "Left or right to seek, centre to pause",
        ),
        Triple(
            "gesture_hold_speed",
            "Press and hold for fast playback",
            "Video plays faster while you hold your finger down",
        ),
        Triple(
            "gesture_video_adjust",
            "Brightness and volume swipes",
            "On video, swipe up or down: left side brightness, right side volume",
        ),
        Triple(
            "gesture_pinch_zoom",
            "Pinch to zoom video",
            "Pinch out to fill the screen, pinch in to fit the whole frame",
        ),
    )

@Composable
private fun SettingToggle(
    title: String,
    value: Boolean,
    description: String = "",
    changed: (Boolean) -> Unit,
) {
    Row(
        Modifier.fillMaxWidth()
            .heightIn(min = 60.dp)
            .clip(MediaTokens.medium)
            .toggleable(value, role = Role.Switch, onValueChange = changed)
            .padding(horizontal = 10.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.Medium)
            if (description.isNotBlank())
                Text(
                    description,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
        }
        MediaSwitch(value)
    }
}

/** Names for the music player styles; the stored ids stay as they are. */
private val PLAYER_STYLE_LABELS =
    mapOf("classic" to "Cover", "vinyl" to "Vinyl", "vu" to "Strings", "orbit" to "Pulse orbit")

private fun choiceLabel(value: String) =
    value.replace("-fullscreen", " · Full screen").replace('-', ' ').replaceFirstChar(Char::titlecase)

@Composable
private fun Choice(
    title: String,
    value: String,
    values: List<String>,
    label: (String) -> String = ::choiceLabel,
    subtitle: String = "",
    changed: (String) -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    Box {
        ValueRow(title, value = label(value), subtitle = subtitle) { open = true }
        if (open)
            Panel(title, { open = false }) {
                values.forEach { item ->
                    SheetAction(
                        text = { Text(label(item)) },
                        leadingIcon = { ChoiceMark(item == value) },
                        modifier = Modifier.semantics { selected = item == value },
                        onClick = {
                            open = false
                            changed(item)
                        },
                    )
                }
            }
    }
}

@Composable
private fun AccountPanel(vm: YtuiViewModel, close: () -> Unit) {
    val account by vm.account.collectAsState()
    var header by remember { mutableStateOf("") }
    val status by vm.status.collectAsState()
    val connected = account.optBoolean("connected")
    Panel("Your account", close) {
        Row(
            Modifier.fillMaxWidth().padding(vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            GlyphBadge(
                if (connected) Glyph.Check else Glyph.Account,
                56.dp,
                if (connected) MaterialTheme.colorScheme.primary
                else MaterialTheme.colorScheme.onSurface,
            )
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(
                    if (connected) "Signed in to YouTube" else "Not signed in",
                    style = MaterialTheme.typography.titleLarge,
                )
                Text(
                    if (connected) "Your session is stored privately on this device."
                    else "Sign in for your subscriptions, playlists and music.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        if (connected)
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                SecondaryAction("Reload session", glyph = Glyph.Refresh) {
                    vm.changeAccount("reload_account")
                }
                SecondaryAction("Sign in again", Modifier.testTag("account-sign-in")) {
                    vm.effect("login")
                }
                QuietAction("Disconnect", destructive = true) { vm.changeAccount("disconnect") }
            }
        else {
            Pill(
                "Sign in to YouTube",
                Modifier.testTag("account-sign-in"),
                PillStyle.Primary,
                fill = true,
            ) {
                vm.effect("login")
            }
        }
        // Importing a session is for people whose sign-in did not work; it stays folded away.
        var otherWays by rememberSaveable { mutableStateOf(false) }
        ValueRow(
            "Other ways to sign in",
            Modifier.testTag("account-other-ways"),
            glyph = Glyph.Folder,
            subtitle = "Import a session from a browser you use",
            chevron = false,
            value = if (otherWays) "Hide" else "Show",
        ) {
            otherWays = !otherWays
        }
        if (otherWays)
            SettingGroup("Import a session") {
                Text(
                    "If Google rejects the sign-in window, import your session instead. Android " +
                        "cannot read another browser's private data, so export it there first. " +
                        "Import only a session you own.",
                    Modifier.padding(horizontal = 10.dp, vertical = 8.dp),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                ValueRow(
                    "Choose cookie file",
                    glyph = Glyph.Folder,
                    subtitle = "Netscape cookie format",
                    modifier = Modifier.testTag("account-import"),
                ) {
                    vm.effect("pick", "kind" to "cookies")
                }
                Column(
                    Modifier.padding(horizontal = 4.dp, vertical = 6.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    MediaTextField(
                        header,
                        { header = it },
                        "Cookie header (optional)",
                        Modifier.testTag("cookie-header-field"),
                        singleLine = false,
                        minLines = 2,
                        visualTransformation = PasswordVisualTransformation(),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                    )
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                        SecondaryAction("Import header", enabled = header.isNotBlank()) {
                            vm.changeAccount("import_cookies", "header" to header)
                            header = ""
                        }
                    }
                }
            }
        SettingGroup("Session & privacy") {
            Row(
                Modifier.padding(horizontal = 10.dp, vertical = 10.dp),
                horizontalArrangement = Arrangement.spacedBy(14.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                MediaIcon(Glyph.Lock, Modifier.size(20.dp), MaterialTheme.colorScheme.onSurfaceVariant)
                Text(
                    "Encrypted with Android Keystore. Keep session files and headers private.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            if (!connected)
                ValueRow("Reload saved session", glyph = Glyph.Refresh, chevron = false) {
                    vm.changeAccount("reload_account")
                }
        }
        if (status != "Ready")
            Text(
                status,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
    }
}

@Composable
private fun FeedPanel(vm: YtuiViewModel, close: () -> Unit) {
    val tab by vm.tab.collectAsState()
    var country by remember { mutableStateOf("") }
    var tag by remember { mutableStateOf("") }
    Panel(if (tab == 3) "Browse stations" else "Feeds and library", close) {
        val choices =
            if (tab == 3) listOf("Featured stations" to "Cliamp", "Saved stations" to "Favorites")
            else
                listOf(
                    "For you" to "Home",
                    "Subscriptions" to "Subscriptions",
                    "Liked" to "Liked",
                    "Watch later" to "WatchLater",
                    "My playlists" to "Playlists",
                    "Library" to "Library",
                    "Saved" to "Favorites",
                )
        for ((title, request) in choices) {
            SheetAction(
                text = { Text(title) },
                leadingIcon = {
                    GlyphTile(
                        when (request) {
                            "Home" -> Glyph.Home
                            "Subscriptions" -> Glyph.Account
                            "WatchLater" -> Glyph.History
                            "Favorites",
                            "Liked" -> Glyph.Save
                            "Cliamp" -> Glyph.Radio
                            else -> Glyph.Queue
                        },
                        Modifier.size(36.dp).clip(MediaTokens.medium),
                    )
                },
                onClick = {
                    vm.navigate(request = request)
                    close()
                },
            )
        }
        if (tab == 3) {
            SectionHeader("By country or genre")
            for ((value, label, button, key) in
                listOf(
                    listOf(
                        country,
                        "Country name or two-letter code",
                        "Browse country",
                        "RadioCountry",
                    ),
                    listOf(tag, "Genre, for example jazz", "Browse genre", "RadioTag"),
                )) {
                Row(
                    Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    MediaTextField(
                        value,
                        { if (key == "RadioCountry") country = it else tag = it },
                        label,
                        Modifier.weight(1f),
                    )
                    RoundIconButton(
                        Glyph.ArrowRight,
                        button,
                        container =
                            if (value.isNotBlank()) MaterialTheme.colorScheme.onBackground
                            else MaterialTheme.colorScheme.onSurface.copy(alpha = 0.07f),
                        tint =
                            if (value.isNotBlank()) MaterialTheme.colorScheme.background
                            else MaterialTheme.colorScheme.onSurfaceVariant,
                        iconSize = 20.dp,
                    ) {
                        if (value.isNotBlank()) {
                            vm.navigate(request = json(key to value.trim()))
                            close()
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun QueuePanel(vm: YtuiViewModel, close: () -> Unit) {
    val queue by vm.queue.collectAsState()
    val config by vm.config.collectAsState()
    val mobile by vm.mobile.collectAsState()
    // Sheets sit above the app's notice layer, so undo lives inside the sheet.
    var undo by remember { mutableStateOf<Pair<String, () -> Unit>?>(null) }
    LaunchedEffect(undo) {
        if (undo != null) {
            delay(6_000)
            undo = null
        }
    }
    Panel("Playback queue", close, scroll = false) {
        undo?.let { (text, restore) ->
            Row(
                Modifier.fillMaxWidth()
                    .clip(MediaTokens.medium)
                    .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                    .padding(start = 14.dp)
                    .testTag("queue-undo"),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(text, Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium)
                QuietAction("Undo") {
                    restore()
                    undo = null
                }
            }
        }
        if (queue.isEmpty())
            EmptyMediaState(
                Glyph.Queue,
                "Queue is empty",
                "Add a video or song to play it next.",
                Modifier.testTag("queue-empty"),
            )
        else {
            Row(
                Modifier.fillMaxWidth().padding(start = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    if (queue.size == 1) "1 item · drag to reorder, swipe to remove"
                    else "${queue.size} items · drag to reorder, swipe to remove",
                    Modifier.weight(1f),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                QuietAction("Clear queue", Modifier.testTag("queue-clear"), destructive = true) {
                    val saved = queue
                    vm.clearQueue(saved)
                    undo = "Queue cleared" to { vm.requeue(saved) }
                }
            }
            ReorderableList(
                queue,
                key = { it.key },
                onMove = { from, to -> vm.moveQueue(from, to, queue[from]) },
                modifier = Modifier.weight(1f, fill = false).fillMaxWidth().testTag("queue-list"),
                swipeEnabled = mobile.optBoolean("gestures", true),
                onSwipeRemove = { index, media ->
                    vm.removeQueue(index, media)
                    undo = "Removed ${media.title}" to { vm.restoreQueue(index, media) }
                },
            ) { index, media, handle ->
                var actions by remember(media.key) { mutableStateOf(false) }
                Row(
                    Modifier.fillMaxWidth().testTag("queue-item-${media.key}"),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Box(handle.size(40.dp, 56.dp), contentAlignment = Alignment.Center) {
                        MediaIcon(
                            Glyph.Drag,
                            Modifier.size(20.dp),
                            MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    Box(Modifier.weight(1f)) {
                        LibraryMediaRow(
                            media,
                            config.optBoolean("thumbnails", true),
                            mobile.optBoolean("show_channel", true),
                        ) {
                            vm.play(media, queued = true)
                            close()
                        }
                    }
                    Box {
                        MediaIconButton(Glyph.More, "Queue actions", "queue-menu-${media.key}") {
                            actions = true
                        }
                        MediaMenu(actions, { actions = false }) {
                            MediaMenuItem("Play", Glyph.Play) {
                                actions = false
                                vm.play(media, queued = true)
                                close()
                            }
                            MediaMenuItem(
                                "Move up",
                                Glyph.Up,
                                Modifier.testTag("queue-up-${media.key}").semantics {
                                    contentDescription = "Move up"
                                },
                                enabled = index > 0,
                            ) {
                                actions = false
                                vm.moveQueue(index, index - 1, media)
                            }
                            MediaMenuItem(
                                "Move down",
                                Glyph.Down,
                                Modifier.testTag("queue-down-${media.key}").semantics {
                                    contentDescription = "Move down"
                                },
                                enabled = index + 1 < queue.size,
                            ) {
                                actions = false
                                vm.moveQueue(index, index + 1, media)
                            }
                            MediaMenuItem(
                                "Remove",
                                Glyph.Close,
                                Modifier.testTag("queue-remove-${media.key}").semantics {
                                    contentDescription = "Remove"
                                },
                                destructive = true,
                            ) {
                                actions = false
                                vm.removeQueue(index, media)
                                undo = "Removed ${media.title}" to { vm.restoreQueue(index, media) }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun HistoryPanel(vm: YtuiViewModel, close: () -> Unit) {
    val history by vm.recent.collectAsState()
    val config by vm.config.collectAsState()
    val mobile by vm.mobile.collectAsState()
    Panel("History", close, scroll = false) {
        val context = androidx.compose.ui.platform.LocalContext.current
        QuietAction("Clear history and resume positions", destructive = true) {
            vm.command("history_clear")
            RecentSearches.clear(context)
        }
        if (history.isEmpty())
            EmptyMediaState(
                Glyph.History,
                "No history yet",
                "Videos and songs you play appear here.",
            )
        else
            LazyColumn(
                Modifier.weight(1f, fill = false).fillMaxWidth().testTag("history-list"),
                verticalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                items(history) { media ->
                    LibraryMediaRow(
                        media,
                        config.optBoolean("thumbnails", true),
                        mobile.optBoolean("show_channel", true),
                    ) {
                        vm.play(media, resume = true)
                        close()
                    }
                }
            }
    }
}

/** Shared quiet list presentation; caller retains its original play/resume action. */
@Composable
private fun LibraryMediaRow(
    media: MediaCard,
    thumbnails: Boolean,
    showCreator: Boolean,
    play: () -> Unit,
) =
    MediaListRow(
        media.title,
        if (showCreator) media.creator else "",
        artwork = { Artwork(media, it, thumbnails) },
        square = media.audio,
        onClick = play,
    )

@Composable
private fun LyricsPanel(vm: YtuiViewModel, close: () -> Unit) {
    val player by vm.player.collectAsState()
    val stored by vm.lyricData.collectAsState()
    val versions by vm.lyricVersions.collectAsState()
    val imported by vm.importedLyrics.collectAsState()
    val searching by vm.lyricSearching.collectAsState()
    val media = remember { player.media?.copyJson() ?: JSONObject() }
    var title by remember { mutableStateOf(media.optString("title")) }
    var artist by remember { mutableStateOf(media.optString("creator")) }
    var text by remember { mutableStateOf("") }
    var offset by remember { mutableStateOf("0") }
    var selected by remember { mutableIntStateOf(0) }
    var follow by remember { mutableStateOf(true) }
    val list = rememberLazyListState()
    val lines =
        remember(stored, versions, selected) {
            (stored.optJSONObject("track") ?: versions.getOrNull(selected))
                ?.optJSONArray("lines")
                ?.objects() ?: emptyList()
        }
    val matching = player.media?.optString("url") == media.optString("url")
    val now = player.position * 1000 - (offset.toLongOrNull() ?: 0)
    val active =
        if (matching) lines.indexOfLast { !it.isNull("at_ms") && it.optLong("at_ms") <= now }
        else -1
    DisposableEffect(Unit) { onDispose { vm.lyricClose() } }
    LaunchedEffect(active, follow, matching) {
        if (follow && matching && active >= 0) list.animateScrollToItem(active + 1)
    }
    LaunchedEffect(stored) {
        stored.optJSONObject("document")?.let {
            title = it.optString("title")
            artist = it.optString("artist")
            text = it.optString("text")
            offset = it.optLong("offset_ms").toString()
        }
    }
    LaunchedEffect(imported) {
        imported?.let {
            text = it
            vm.importedLyrics.value = null
        }
    }
    var editing by remember { mutableStateOf(false) }
    val showEditor = editing
    // Between one-second snapshots the playhead is extrapolated per frame, so the sweep glides.
    val smooth = rememberSmoothPosition(player)
    val source =
        stored.optJSONObject("track")?.let { "Saved on this device" }
            ?: versions.getOrNull(selected)?.optString("album")?.takeIf { it.isNotBlank() }
    val timed = lines.any { !it.isNull("at_ms") }
    var menu by remember { mutableStateOf(false) }
    Panel(
        "Lyrics",
        close,
        scroll = false,
        actions = {
            // Settings live behind ⋮ so the panel shows only the song and its lyrics.
            if (lines.isNotEmpty() || editing)
                Box {
                    MediaIconButton(Glyph.More, "Lyrics options", "lyrics-menu") { menu = true }
                    MediaMenu(menu, { menu = false }) {
                        if (lines.isNotEmpty() && source != null)
                            Text(
                                source,
                                Modifier.padding(horizontal = 12.dp, vertical = 8.dp)
                                    .testTag("lyrics-source"),
                                style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        if (timed)
                            MediaMenuItem(
                                if (follow) "Stop auto-scroll" else "Auto-scroll with the song",
                                if (follow) Glyph.Stop else Glyph.Down,
                                Modifier.testTag("lyrics-follow"),
                            ) {
                                follow = !follow
                                menu = false
                            }
                        MediaMenuItem(
                            if (editing) "Hide editor" else "Edit or find lyrics",
                            if (editing) Glyph.Up else Glyph.Edit,
                            Modifier.testTag("lyrics-editor"),
                        ) {
                            editing = !editing
                            menu = false
                        }
                        if (stored.optJSONObject("track") != null) {
                            MenuRule()
                            MediaMenuItem("Remove saved lyrics", Glyph.Close, destructive = true) {
                                vm.lyricRemove(media)
                                menu = false
                            }
                        }
                    }
                }
        },
    ) {
        if (media.length() > 0)
            Column(Modifier.padding(horizontal = 8.dp)) {
                Text(
                    media.optString("title"),
                    style = MaterialTheme.typography.titleMedium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    if (matching) media.optString("creator")
                    else "Timing paused: a different song is playing.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        else Text("Play a song first", Modifier.padding(horizontal = 8.dp))
        if (lines.isEmpty() && searching && !editing)
            Box(
                Modifier.fillMaxWidth().padding(32.dp).testTag("lyrics-loading"),
                contentAlignment = Alignment.Center,
            ) {
                MediaSpinner()
            }
        if (lines.isEmpty() && !searching && !editing && media.length() > 0)
            EmptyMediaState(
                Glyph.Lyrics,
                "No lyrics found",
                "YouTube Music has no lyrics for this song. Search LRCLIB with an edited title and artist, or write your own.",
            ) {
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Action("Find lyrics", "lyrics-find", title.isNotBlank() && artist.isNotBlank()) {
                        vm.lyricLookup(media, title, artist)
                    }
                    QuietAction("Write or import", Modifier.testTag("lyrics-edit")) { editing = true }
                }
            }
        LazyColumn(
            Modifier.weight(1f, fill = false).fillMaxWidth(),
            state = list,
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            item {
                if (editing)
                    SheetAction(
                        text = { Text("Hide editor") },
                        leadingIcon = {
                            MediaIcon(
                                Glyph.Up,
                                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        },
                        onClick = { editing = !editing },
                    )
                if (showEditor)
                    Column(
                        Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
                        verticalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        MediaTextField(title, { title = it }, "Song title")
                        MediaTextField(artist, { artist = it }, "Artist")
                        Text(
                            "Only the reviewed title and artist are sent to LRCLIB when you choose Find.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        FlowRow(
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                            verticalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Action(
                                "Find lyrics",
                                enabled = title.isNotBlank() && artist.isNotBlank(),
                            ) {
                                vm.lyricLookup(media, title, artist)
                            }
                            SecondaryAction("Import LRC / text") { vm.lyricImport() }
                        }
                        if (versions.isNotEmpty()) SectionHeader("Matches")
                        versions.forEachIndexed { index, track ->
                            SheetAction(
                                text = {
                                    Text(
                                        "${track.optString("artist")} · ${track.optString("album")}",
                                        maxLines = 2,
                                        overflow = TextOverflow.Ellipsis,
                                    )
                                },
                                leadingIcon = {
                                    if (index == selected) MediaIcon(Glyph.Check)
                                    else Spacer(Modifier.size(24.dp))
                                },
                                onClick = {
                                    selected = index
                                    vm.lyricData.value = JSONObject()
                                    text =
                                        track.optJSONArray("lines")?.objects()?.joinToString(
                                            "\n"
                                        ) { line ->
                                            if (line.isNull("at_ms")) line.optString("text")
                                            else lrcLine(line)
                                        } ?: ""
                                },
                            )
                        }
                        FieldFrame(
                            text,
                            { if (it.toByteArray().size <= 512 * 1024) text = it },
                            "LRC or plain lyrics",
                            singleLine = false,
                            height = 200.dp,
                        )
                        MediaTextField(
                            offset,
                            { offset = it },
                            "Delay in ms (positive = later)",
                            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                        )
                        FlowRow(
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                            verticalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            SecondaryAction(
                                "Save locally",
                                enabled = media.length() > 0 && text.isNotBlank(),
                            ) {
                                vm.lyricSave(
                                    media,
                                    json(
                                        "title" to title,
                                        "artist" to artist,
                                        "text" to text,
                                        "offset_ms" to (offset.toLongOrNull() ?: 0),
                                    ),
                                )
                                editing = false
                            }
                            QuietAction("Remove saved lyrics", destructive = true) {
                                vm.lyricRemove(media)
                            }
                        }
                    }
            }
            itemsIndexed(lines) { index, line ->
                val current = index == active
                val ink = MaterialTheme.colorScheme.onSurface
                val unsung = ink.copy(alpha = LYRIC_UNSUNG_ALPHA)
                // Karaoke sweep: only lines with word timings fill as they are sung; others
                // light up whole, as in YouTube Music.
                val sweep =
                    if (!current || !matching) null
                    else
                        lyricSweep(
                            smooth * 1000.0 - (offset.toLongOrNull() ?: 0),
                            line.optJSONArray("words")?.objects()?.map {
                                LyricWord(it.optLong("at_ms"), it.optString("text"))
                            } ?: emptyList(),
                            line.takeUnless { it.isNull("end_ms") }?.optLong("end_ms"),
                            lines.getOrNull(index + 1)?.takeUnless { it.isNull("at_ms") }?.optLong("at_ms"),
                        )
                val text = line.optString("text").ifBlank { "♪" }
                Text(
                    if (sweep != null && LocalMotion.current.animate)
                        karaokeLine(text, sweep, ink, unsung)
                    else AnnotatedString(text),
                    Modifier.fillMaxWidth()
                        .clip(MediaTokens.medium)
                        .clickable(enabled = matching && !line.isNull("at_ms")) {
                            vm.command(
                                "seek",
                                "seconds" to
                                    ((line.optLong("at_ms") + (offset.toLongOrNull() ?: 0))
                                        .coerceAtLeast(0) / 1000.0),
                            )
                        }
                        .padding(horizontal = 8.dp, vertical = 6.dp),
                    color =
                        animateColorAsState(
                            if (current || active < 0) ink else unsung,
                            LocalMotion.current.fade(320),
                            label = "lyric-line",
                        ).value,
                    style = MaterialTheme.typography.headlineSmall,
                )
            }
        }
    }
}

/** Tablet layout test shared by the shell and notices (same 840dp breakpoint). */
@Composable
internal fun tabletLayoutOf(): Boolean = LocalConfiguration.current.screenWidthDp >= 840

/** Playback position extrapolated per frame between snapshots while playing. */
@Composable
internal fun rememberSmoothPosition(player: PlayerState): Float {
    var base by remember { mutableStateOf(player.position to android.os.SystemClock.elapsedRealtime()) }
    LaunchedEffect(player.position) { base = player.position to android.os.SystemClock.elapsedRealtime() }
    var now by remember { mutableLongStateOf(android.os.SystemClock.elapsedRealtime()) }
    val animate = LocalMotion.current.animate
    LaunchedEffect(player.playing, animate) {
        // An infinite-animation frame loop: paused by test and accessibility animation policies.
        while (player.playing && animate)
            withInfiniteAnimationFrameNanos { now = android.os.SystemClock.elapsedRealtime() }
    }
    return if (!player.playing) player.position
    else (base.first + (now - base.second).coerceIn(0, 1500) / 1000f)
}
