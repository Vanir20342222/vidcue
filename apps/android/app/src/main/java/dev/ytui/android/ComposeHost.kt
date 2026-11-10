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
import androidx.compose.animation.core.tween
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
            MaterialTheme(colorScheme = scheme, typography = typography) {
                CompositionLocalProvider(
                    LocalContentColor provides scheme.onBackground,
                    LocalIndication provides SoftIndication,
                    LocalRippleConfiguration provides null,
                    LocalMotion provides Motion(reduced = AuthUi.reducedMotion),
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
            val initial =
                (feed.value.request as? JSONObject)?.has(
                    if (tab == 3) "RadioSearch" else "Search"
                ) == true
            feed
                .map {
                    (it.request as? JSONObject)?.has(if (tab == 3) "RadioSearch" else "Search") ==
                        true
                }
                .distinctUntilChanged() to initial
        }
    val searchResults =
        key(tab) { searchRequestState.first.collectAsState(searchRequestState.second).value }
    val searchMode = searchFocused || searchResults
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
    var menu by remember { mutableStateOf(false) }
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
                            Row(
                                Modifier.fillMaxWidth()
                                    .testTag("app-toolbar")
                                    .heightIn(min = 64.dp)
                                    .padding(start = 8.dp, end = 4.dp, top = 4.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                // The place you are, set in the display face. It replaces
                                // the wordmark: the launcher already names the app.
                                PageTitle(
                                    when (tab) {
                                        0 -> "Downloads"
                                        2 -> "Music"
                                        3 -> "Radio"
                                        else -> "Home"
                                    },
                                    Modifier.padding(start = 8.dp).weight(1f),
                                )
                                AccountButton(account.optBoolean("connected")) { dialog = "account" }
                                MediaIconButton(Glyph.Settings, "Settings", "settings-button") {
                                    dialog = "settings"
                                }
                                Box {
                                    MediaIconButton(Glyph.More, "More options", "menu-button") {
                                        menu = true
                                    }
                                    MediaMenu(menu, { menu = false }) {
                                        // Feed actions first, then player panels, then About;
                                        // every row has its own glyph so none read alike.
                                        if (tab > 0) {
                                            MediaMenuItem(
                                                "Browse",
                                                if (tab == 3) Glyph.Radio else Glyph.Browse,
                                                Modifier.testTag("feeds-button").semantics {
                                                    contentDescription = "Browse"
                                                },
                                            ) {
                                                menu = false
                                                dialog = "feeds"
                                            }
                                            MediaMenuItem("Refresh feed", Glyph.Refresh) {
                                                menu = false
                                                vm.navigate(
                                                    request = vm.feeds[tab].value.request,
                                                    remember = false,
                                                )
                                            }
                                            MenuRule()
                                        }
                                        for ((label, name, icon) in
                                            listOf(
                                                Triple("Playback queue", "queue", Glyph.UpNext),
                                                Triple("Recently played", "history", Glyph.History),
                                                Triple("Lyrics", "lyrics", Glyph.Lyrics),
                                                Triple("Sleep timer", "sleep", Glyph.Sleep),
                                                Triple("About", "about", Glyph.Info),
                                            )) {
                                            if (name == "about") MenuRule()
                                            MediaMenuItem(label, icon) {
                                                menu = false
                                                dialog = name
                                                if (name == "queue") vm.loadQueue()
                                                if (name == "history") vm.loadHistory()
                                                if (name == "lyrics")
                                                    vm.player.value.media?.let(vm::lyricLoad)
                                            }
                                        }
                                    }
                                }
                            }
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
                                if (tablet) AppNavigation(tab, ::selectTab, vertical = true)
                                Column(Modifier.weight(1f).fillMaxHeight()) {
                                if (tablet) Toolbar()
                                Box(
                                    Modifier.weight(1f)
                                        .fillMaxWidth()
                                        .tabSwipe(tab, enabled = mobile.gesture("gesture_swipe_tabs") && !searchMode) {
                                            vm.chooseTab(it)
                                        }
                                ) {
                                    if (tab == 0) OfflineLibrary(vm)
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
                                        .testTag("bottom-dock")
                                ) {
                                    PlayerDock(vm)
                                    AppNavigation(tab, ::selectTab)
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
                            PlaybackPanel(allowVideoSurface = false)
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
                            )
                        }
                    "settings" ->
                        SettingsPanel(
                            vm,
                            dark,
                            onAccount = { dialog = "account" },
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
                                "${BuildConfig.VERSION_NAME} (${BuildConfig.VERSION_CODE})\nRust engine · native Android UI\nSource ${BuildConfig.SOURCE_REVISION}"
                            )
                            Text(
                                "All provider, extraction, account and library operations use the shared Rust engine. Android supplies touch, keyboard, video and background media. Development preview. Device playback, network and update checks are tracked in the validation report."
                            )
                            AppUpdateSettings()
                            ValueRow("Radio artwork credits", glyph = Glyph.Link) {
                                vm.effect(
                                    "open",
                                    "url" to
                                        "https://github.com/Vanir20342222/ytui/blob/main/docs/android/RADIO_ARTWORK.md",
                                )
                            }
                            ValueRow("Mobile documentation", glyph = Glyph.Link) {
                                vm.effect(
                                    "open",
                                    "url" to
                                        "https://github.com/Vanir20342222/ytui/tree/main/docs/android",
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
                val reducedMotion = config.optBoolean("reduced_motion")
                AnimatedVisibility(
                    visible = onboarding,
                    enter = EnterTransition.None,
                    exit =
                        if (reducedMotion) ExitTransition.None
                        else fadeOut(tween(280)) + scaleOut(tween(320), targetScale = 1.04f),
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
 * order (Home, Music, Radio, Downloads). The page follows the finger; rows that scroll sideways
 * themselves (filter chips, shelves) keep their own drags.
 */
@Composable
private fun Modifier.tabSwipe(tab: Int, enabled: Boolean, choose: (Int) -> Unit): Modifier {
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
            allow = { neighbourTab(current, -it) != null },
            onSwipe = { direction ->
                val next = neighbourTab(current, -direction) ?: return@swipeAxis
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
    val wideSearch =
        tab == 1 &&
            LocalConfiguration.current.screenWidthDp >= 840 &&
            (feed.request as? JSONObject)?.has("Search") == true
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
    val searchKey = if (tab == 3) "RadioSearch" else "Search"
    val searchResults = (feed.request as? JSONObject)?.has(searchKey) == true
    val searchMode = searchActive || searchResults
    val showFeedBack = feed.request != "Home" && feed.request != "Cliamp" && !searchResults
    LaunchedEffect(feed.request) {
        (feed.request as? JSONObject)
            ?.optString(searchKey)
            ?.takeIf { it.isNotBlank() }
            ?.let { query = it }
    }
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
    val pageItemCount =
        (shelves?.let { it.size * 2 } ?: feed.cards.size) +
            (if (showFeedBack) 1 else 0) + (if (feed.notice != null) 1 else 0)
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
            val fixedColumns = tab == 2 && LocalConfiguration.current.screenWidthDp < 600
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
                verticalArrangement = Arrangement.spacedBy(if (tab in 2..3) 20.dp else 28.dp),
            ) {
                if (showFeedBack)
                    item(span = { GridItemSpan(maxLineSpan) }) {
                        Box(Modifier.padding(horizontal = 4.dp), contentAlignment = Alignment.CenterStart) {
                            Pill(
                                "Back to previous feed",
                                style = PillStyle.Quiet,
                                glyph = Glyph.Back,
                            ) {
                                vm.back()
                            }
                        }
                    }
                if (feed.notice != null)
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
                            if (needsAccount) "Account needed" else "This feed is unavailable",
                            feed.error!!,
                        ) {
                            FlowRow(
                                horizontalArrangement =
                                    Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
                                verticalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                if (needsAccount) {
                                    Action("Sign in / import cookies", "feed-account-action") {
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
                        item(
                            span = { GridItemSpan(maxLineSpan) },
                            key = "shelf-head:$section",
                            contentType = "shelf-head",
                        ) {
                            val inset = if (tab == 1) MediaTokens.textInset else 0.dp
                            Row(
                                Modifier.fillMaxWidth().padding(start = inset, end = inset, top = 4.dp),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(12.dp),
                            ) {
                                Text(
                                    section.ifBlank { "More for you" },
                                    Modifier.weight(1f)
                                        .semantics { heading() }
                                        .testTag("shelf-$section"),
                                    style = MaterialTheme.typography.titleLarge,
                                )
                                // Albums and playlists open as their own pages, so a shelf of
                                // only collections or live entries has nothing to play in order.
                                if (cards.any { !it.collection && !it.live && it.url.isNotBlank() })
                                    // Icon only, sized to the title; the label stays for TalkBack.
                                    RoundIconButton(
                                        Glyph.Play,
                                        "Play all ${section.ifBlank { "More for you" }}",
                                        Modifier.border(
                                                1.dp,
                                                MaterialTheme.colorScheme.outlineVariant,
                                                CircleShape,
                                            )
                                            .testTag("shelf-play-all-$section"),
                                        size = 40.dp,
                                        iconSize = 20.dp,
                                    ) {
                                        vm.playAll(cards)
                                    }
                            }
                        }
                        item(
                            span = { GridItemSpan(maxLineSpan) },
                            key = "shelf:$section",
                            contentType = "shelf",
                        ) {
                            LazyRow(
                                Modifier.fillMaxWidth(),
                                contentPadding =
                                    PaddingValues(horizontal = if (tab == 1) MediaTokens.textInset else 0.dp),
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
                    span = { GridItemSpan(if (wideSearch) maxLineSpan else 1) },
                    contentType = { "media-card" },
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
                        EmptyMediaState(
                            if (tab == 2) Glyph.Music
                            else if (tab == 3) Glyph.Radio else Glyph.Search,
                            if (tab == 2) "No music loaded"
                            else if (tab == 3) "No stations loaded" else "No videos loaded",
                            if (tab == 3)
                                "Search a station, explore a genre or browse cliamp channels."
                            else
                                "Search above or connect your YouTube account for a feed made for you.",
                        ) {
                            Action(if (tab == 3) "Browse stations" else "Explore feeds") {
                                onFeeds()
                            }
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
        }
        Column(
            Modifier.fillMaxWidth()
                .onSizeChanged { headerHeight = it.height }
                .graphicsLayer { translationY = if (searchActive) 0f else -headerOffset.floatValue }
                .background(MaterialTheme.colorScheme.background)
                .padding(bottom = 12.dp)
                .testTag("browse-header"),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
        Row(
            Modifier.align(Alignment.CenterHorizontally)
                .padding(horizontal = MediaTokens.textInset)
                .widthIn(max = 720.dp)
                .fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (searchMode)
                MediaIconButton(Glyph.Back, "Leave search") {
                    releaseSearchFocus()
                    if (searchResults) vm.back()
                }
            SearchField(
                query,
                { query = it },
                if (tab == 3) "Search stations or genres" else "Search or paste a link",
                // A single-line input should not stretch across a wide canvas;
                // cap it and center it when the pane is wider than the cap.
                Modifier.weight(1f)
                    .focusRequester(focus)
                    .onFocusChanged { onSearchFocus(it.isFocused) }
                    .testTag("search-field"),
                searching = searchMode,
                onClear = {
                    query = ""
                    focus.requestFocus()
                    keyboard?.show()
                },
                onSubmit = {
                    keyboard?.hide()
                    if (rememberSearches) RecentSearches.add(context, tab == 3, query)
                    vm.search(query)
                },
            )
        }
        if (tab in 1..2 || feed.categories.isNotEmpty())
            LazyRow(
                Modifier.fillMaxWidth().testTag("feed-filters"),
                contentPadding = PaddingValues(horizontal = MediaTokens.textInset),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                if (tab in 1..2)
                    items(
                        // Library feeds the Browse sheet already offers, surfaced
                        // where people look for them (Home only for video feeds).
                        listOfNotNull(
                            "For you" to "Home",
                            "My playlists" to "Playlists",
                            "Liked" to "Liked",
                            if (tab == 1) "Subscriptions" to "Subscriptions" else null,
                            if (tab == 1) "Watch later" to "WatchLater" else null,
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
                items(feed.categories) { category ->
                    MediaChip(
                        category.optString("title"),
                        modifier = Modifier.testTag("category-${category.optString("title")}"),
                        selected =
                            feed.request !in
                                listOf(
                                    "Home",
                                    "Subscriptions",
                                    "WatchLater",
                                    "Playlists",
                                    "Liked",
                                ) && category.opt("request")?.toString() == feed.request.toString(),
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
    // Feed video art is full bleed; in a shelf it is a rounded object like the music covers.
    val bleed = !square && !shelf
    val artShape = if (bleed) RoundedCornerShape(0.dp) else MediaTokens.medium
    Column(Modifier.fillMaxWidth().testTag("media-card-${media.key}")) {
        Box(
            Modifier.fillMaxWidth()
                .aspectRatio(if (square) 1f else 16f / 9f)
                .clip(artShape)
        ) {
            Artwork(
                media,
                Modifier.fillMaxSize()
                    .clickable { vm.play(media) }
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
            Modifier.fillMaxWidth()
                .padding(
                    start = if (bleed) 16.dp else 0.dp,
                    end = if (bleed) 4.dp else 0.dp,
                    top = 8.dp,
                ),
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
                    .clickable { vm.play(media) }
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
                        media.creator.ifBlank {
                            if (media.collection) media.kind.replaceFirstChar(Char::titlecase)
                            else if (tab == 3) "Radio station" else "YouTube"
                        },
                        Modifier.opensChannel(media.creator, media.channelId) { vm.openChannel(it) },
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                CardInformation(media.value, mobile)
            }
            MediaCardActions(media) { openMenu(media, square) }
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
                if (media.collection) "Open collection" else "Play now",
                Modifier.weight(1f),
                primary = true,
            ) {
                execute { vm.play(media) }
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
            if (!media.collection && !media.live)
                QuickTile(Glyph.Preview, "Preview", Modifier.weight(1f)) {
                    execute { vm.play(media, preview = true) }
                }
        }
        if (media.collection)
            ValueRow("Play all", glyph = Glyph.Queue, chevron = false) {
                execute { vm.playCollection(media) }
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
        ValueRow(
            "Bookmark",
            glyph = Glyph.Save,
            subtitle = "Keep a link in Saved locally, without downloading",
            chevron = false,
        ) {
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
                    SeekJump("−$seekSeconds", "Back $seekSeconds seconds") { vm.command("back") }
                    PlayDisc(player.playing, Modifier.testTag("player-pause"), size = 64.dp) {
                        vm.command("pause")
                    }
                    SeekJump("+$seekSeconds", "Forward $seekSeconds seconds") { vm.command("forward") }
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
                    MediaIconButton(Glyph.UpNext, "Playback queue") { onQueue() }
                    MediaIconButton(Glyph.Stop, "Stop playback") { vm.command("stop") }
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
                        .shadow(18.dp, RoundedCornerShape(16.dp), ambientColor = Color.Black, spotColor = Color.Black)
                        .clip(RoundedCornerShape(16.dp)),
                    config.optBoolean("thumbnails", true),
                    shape = RoundedCornerShape(16.dp),
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
    close: () -> Unit,
) {
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
        if (twoPane) "Settings" else category.ifBlank { "Settings" },
        close,
        page = true,
        scroll = !twoPane,
        headerBack = if (!twoPane && category.isNotBlank()) ({ category = "" }) else null,
        headerBackTag = "settings-back",
    ) {
        BackHandler(category.isNotBlank() && !twoPane) { category = "" }
        @Composable
        fun RootList() {
            for ((group, rows) in
                listOf(
                    "Look and feel" to
                        listOf(
                            Triple("Appearance", Glyph.Palette, "Theme, motion, music player, gestures"),
                            Triple("What cards show", Glyph.Info, "Channel, views, dates and more on each card"),
                        ),
                    "Listening" to
                        listOf(
                            Triple("Playback", Glyph.Play, "Quality, SponsorBlock, preloading, seeking"),
                            Triple("Offline & storage", Glyph.Download, "Audio-only saves and folders"),
                        ),
                    "Connection" to
                        listOf(
                            Triple("Account", Glyph.Account, "Sign in or import a session"),
                            Triple("Network", Glyph.Link, "Proxy"),
                        ),
                    "This device" to
                        listOf(
                            Triple("Privacy", Glyph.History, "History and resume"),
                            Triple("Performance", Glyph.Speed, "Startup timing report"),
                            Triple("Setup guide", Glyph.Play, "Replay the welcome and setup steps"),
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
                                "Account" -> onAccount()
                                "Setup guide" -> onSetup()
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
                        leadingIcon = {
                            GlyphTile(Glyph.Palette, Modifier.size(36.dp).clip(MediaTokens.medium))
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
                    glyph = Glyph.Edit,
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
                    "Reduced motion",
                    config.optBoolean("reduced_motion"),
                    "Keep visualizers still and turn off all animations",
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
                ) { value ->
                    vm.updateConfig(dark) { it.getJSONObject("download").put("quality", value) }
                }
                Text(
                    "Also limits videos you save offline.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
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
        if (shown == "Performance") {
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
                        "Reduced motion is on, so animations are off. These choices apply when it is off.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                Choice(
                    "Animation style",
                    mobile.optString("animation_style", "smooth"),
                    listOf("smooth", "snappy", "bouncy", "gentle"),
                ) {
                    vm.updateMobile("animation_style", it)
                }
                Text(
                    when (mobile.optString("animation_style", "smooth")) {
                        "snappy" -> "Short, crisp transitions"
                        "bouncy" -> "Springy, with a little overshoot"
                        "gentle" -> "Longer, softer transitions"
                        else -> "Fluid ease-out transitions (default)"
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Choice(
                    "Animation speed",
                    animationSpeedLabel(mobile.optInt("animation_speed", 100)),
                    ANIMATION_SPEEDS.map { it.second },
                ) { label ->
                    ANIMATION_SPEEDS.firstOrNull { it.second == label }?.let {
                        vm.updateMobile("animation_speed", it.first)
                    }
                }
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
        if (shown == "Appearance")
            SettingGroup("Gestures") {
                val gesturesOn = mobile.optBoolean("gestures", true)
                SettingToggle("Swipe gestures", gesturesOn, "Master switch for every gesture below") {
                    vm.updateMobile("gestures", it)
                }
                val context = androidx.compose.ui.platform.LocalContext.current
                ValueRow(
                    "Show gesture tips again",
                    glyph = Glyph.Info,
                    subtitle = "The mini player shows how to swipe it next time it appears",
                    chevron = false,
                ) {
                    GestureTips.reset(context)
                    vm.confirm("tips")
                }
                if (gesturesOn) {
                    for ((key, title, detail) in GESTURE_SETTINGS)
                        SettingToggle(title, mobile.optBoolean(key, true), detail) {
                            vm.updateMobile(key, it)
                        }
                    if (mobile.optBoolean("gesture_hold_speed", true))
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
                        "preload_mobile" to "Preload on mobile data",
                        "autoplay_related" to "Related media autoplay",
                        "resume_on_play" to "Resume videos where you left off",
                        "prefer_offline" to "Prefer saved offline media",
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
                    "Color grading",
                    mobile.optString("color_grade", "off"),
                    ColorGradeMath.PRESETS,
                ) {
                    vm.updateMobile("color_grade", it)
                }
                SettingToggle(
                    "Auto color grade",
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
                Choice(
                    "Streaming cache (MiB)",
                    mobile.optInt("stream_cache_mb", 128).toString(),
                    listOf("32", "64", "128", "256", "512"),
                ) {
                    vm.updateMobile("stream_cache_mb", it.toInt())
                }
                Text(
                    "Cache size applies after restarting the app. Streaming cache is separate from your offline library.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        if (shown == "Offline & storage")
            SettingGroup("Offline media", showTitle = false) {
                SettingToggle(
                    "Audio-only saves",
                    config.optJSONObject("download")?.optBoolean("audio") == true,
                    "Save YouTube Music without video. YouTube videos always keep video.",
                ) { value ->
                    vm.updateConfig(dark) { it.getJSONObject("download").put("audio", value) }
                }
                Text(
                    "Video quality for saves follows Playback › Maximum video quality.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
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
                    "Save completed media to selected folders",
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
                        glyph = if (audio) Glyph.Music else Glyph.Preview,
                        subtitle = mobile.optString(key, "App storage").ifBlank { "App storage" },
                    ) {
                        vm.effect("folder_pick", "audio" to audio)
                    }
                }
                ValueRow("Retry folder saves", glyph = Glyph.Repeat, chevron = false) {
                    vm.effect("folder_retry")
                }
            }
        if (shown == "What cards show")
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

private val GESTURE_SETTINGS =
    listOf(
        Triple(
            "gesture_swipe_tabs",
            "Swipe between tabs",
            "Swipe sideways to move between Home, Music, Radio and Downloads",
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
            "Video: left or right to seek, centre to pause · Music artwork: pause",
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
    changed: (String) -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    Box {
        ValueRow(title, value = label(value)) { open = true }
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
            Text(
                "Google may reject embedded sign-in. You can also import your session below.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        SettingGroup("Import a session") {
            Text(
                "Android cannot read another browser's private data. Import only a session you own.",
                Modifier.padding(horizontal = 10.dp, vertical = 8.dp),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            ValueRow(
                "Choose cookie file",
                glyph = Glyph.Link,
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
    Panel("Feeds and library", close) {
        val choices =
            if (tab == 3) listOf("Cliamp channels" to "Cliamp", "Saved stations" to "Favorites")
            else
                listOf(
                    "For you" to "Home",
                    "Subscriptions" to "Subscriptions",
                    "Liked" to "Liked",
                    "Watch later" to "WatchLater",
                    "My playlists" to "Playlists",
                    "Library" to "Library",
                    "Saved locally" to "Favorites",
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
            SectionHeader("Browse by country or genre")
            for ((value, label, button, key) in
                listOf(
                    listOf(
                        country,
                        "Country name or two-letter code",
                        "Browse country",
                        "RadioCountry",
                    ),
                    listOf(tag, "Radio genre / tag", "Browse genre", "RadioTag"),
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
    Panel("Recently played", close, scroll = false) {
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
