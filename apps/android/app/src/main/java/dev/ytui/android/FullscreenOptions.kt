package dev.ytui.android

import android.view.View
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.platform.ViewCompositionStrategy
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import androidx.media3.ui.PlayerView
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

/**
 * Every playback option from the main player, inside fullscreen. The panel is a Compose layer in
 * the fullscreen window itself (no dialog window), so system bars stay hidden and the shared
 * decoder keeps rendering behind it. Options reuse [PlayerPane] and the same Rust commands.
 */
@androidx.annotation.OptIn(markerClass = [androidx.media3.common.util.UnstableApi::class])
internal object FullscreenOptions {
    /** Test hook: whether the fullscreen options panel is currently open. */
    @JvmStatic
    val open = MutableStateFlow(false)

    @JvmStatic
    fun attach(activity: ComponentActivity, view: PlayerView): View {
        open.value = false
        view.findViewById<View>(R.id.ytui_heading)?.visibility = View.VISIBLE
        view.findViewById<View>(R.id.ytui_settings)?.apply {
            visibility = View.VISIBLE
            setOnClickListener {
                view.hideController()
                open.value = true
            }
        }
        // Share the main screen's state (settings, queue, palette) when it is alive; a
        // standalone model still drives the same playback service through the bridge.
        val owner: ComponentActivity =
            RustActivity.current()?.takeUnless { it.isDestroyed || it.isFinishing } ?: activity
        val vm = ViewModelProvider(owner)[YtuiViewModel::class.java]
        return ComposeView(activity).apply {
            setViewCompositionStrategy(ViewCompositionStrategy.DisposeOnViewTreeLifecycleDestroyed)
            setContent { FullscreenOptionsLayer(vm, view, activity::finish) }
        }
    }
}

@androidx.annotation.OptIn(markerClass = [androidx.media3.common.util.UnstableApi::class])
@Composable
private fun FullscreenOptionsLayer(vm: YtuiViewModel, view: PlayerView, exit: () -> Unit) {
    val visible by FullscreenOptions.open.collectAsState()
    val player by vm.player.collectAsState()
    val config by vm.config.collectAsState()
    val mobile by vm.mobile.collectAsState()
    val queue by vm.queue.collectAsState()
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    // The main activity is stopped beneath fullscreen, so this window keeps the
    // player snapshot current (same cadence and signal as the main screen).
    LaunchedEffect(lifecycle) {
        lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) {
            coroutineScope {
                launch { ComposeHost.signals.collect { vm.snapshot() } }
                launch {
                    while (isActive) {
                        vm.snapshot()
                        delay(1000)
                    }
                }
            }
        }
    }
    val title = player.title
    val creator =
        player.value.optString("station").ifBlank { player.media?.optString("creator") ?: "" }
    SideEffect {
        view.findViewById<TextView>(R.id.ytui_title)?.text = title
        view.findViewById<TextView>(R.id.ytui_subtitle)?.apply {
            text = creator
            visibility = if (creator.isBlank()) View.GONE else View.VISIBLE
        }
    }
    val scheme = ComposeHost.colorScheme ?: darkColorScheme()
    val reduced =
        config.optBoolean("reduced_motion") ||
            !android.animation.ValueAnimator.areAnimatorsEnabled()
    var page by remember { mutableStateOf("") }
    fun close() {
        FullscreenOptions.open.value = false
        page = ""
    }
    BackHandler(visible) { if (page.isNotEmpty()) page = "" else close() }
    MaterialTheme(colorScheme = scheme, typography = MediaTokens.typography) {
        CompositionLocalProvider(
            LocalContentColor provides scheme.onSurface,
            LocalReducedMotion provides reduced,
        ) {
            BoxWithConstraints(Modifier.fillMaxSize()) {
                val wide = maxWidth > maxHeight
                val sheetWidth = if (wide) minOf(420.dp, maxWidth * 0.5f) else maxWidth
                AnimatedVisibility(
                    visible,
                    enter = if (reduced) EnterTransition.None else fadeIn(tween(160)),
                    exit = if (reduced) ExitTransition.None else fadeOut(tween(120)),
                ) {
                    // Tapping the dimmed video closes the panel; it never reaches the player.
                    Box(
                        Modifier.fillMaxSize()
                            .background(Color.Black.copy(alpha = 0.45f))
                            .testTag("fullscreen-options-scrim")
                            .pointerInput(Unit) { detectTapGestures { close() } }
                    )
                }
                AnimatedVisibility(
                    visible,
                    Modifier.align(Alignment.CenterEnd),
                    enter =
                        if (reduced) EnterTransition.None
                        else slideInHorizontally(tween(220)) { it / 3 } + fadeIn(tween(160)),
                    exit =
                        if (reduced) ExitTransition.None
                        else slideOutHorizontally(tween(160)) { it / 3 } + fadeOut(tween(120)),
                ) {
                    Surface(
                        Modifier.width(sheetWidth)
                            .fillMaxHeight()
                            .testTag("fullscreen-options")
                            .semantics { paneTitle = "Playback options" },
                        shape = RoundedCornerShape(topStart = 24.dp, bottomStart = 24.dp),
                        color = scheme.surfaceContainerLow,
                        tonalElevation = 2.dp,
                    ) {
                        Column(
                            Modifier.fillMaxSize()
                                .windowInsetsPadding(
                                    WindowInsets.displayCutout.only(
                                        WindowInsetsSides.End + WindowInsetsSides.Vertical
                                    )
                                )
                                .padding(start = 8.dp, end = 8.dp, top = 8.dp)
                        ) {
                            Row(
                                Modifier.fillMaxWidth().heightIn(min = 56.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                if (page.isNotEmpty())
                                    MediaIconButton(
                                        Glyph.Back,
                                        "Back to playback options",
                                        "fullscreen-options-back",
                                    ) {
                                        page = ""
                                    }
                                Text(
                                    when (page) {
                                        "sleep" -> "Sleep timer"
                                        "queue" -> "Up next"
                                        else -> "Playback options"
                                    },
                                    Modifier.weight(1f).padding(horizontal = 8.dp),
                                    style = MaterialTheme.typography.titleLarge,
                                    fontWeight = FontWeight.Medium,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                                MediaIconButton(
                                    Glyph.Close,
                                    "Close playback options",
                                    "fullscreen-options-close",
                                ) {
                                    close()
                                }
                            }
                            HorizontalDivider(color = scheme.outlineVariant)
                            when (page) {
                                "queue" ->
                                    UpNext(
                                        queue,
                                        player.media?.optString("url"),
                                        config.optBoolean("thumbnails", true),
                                        mobile.optBoolean("show_channel", true),
                                        Modifier.weight(1f),
                                        onManage = null,
                                        showHeader = false,
                                    ) {
                                        vm.play(it, queued = true)
                                    }
                                "sleep" ->
                                    Column(Modifier.verticalScroll(rememberScrollState())) {
                                        SleepOptions(vm) { page = "" }
                                    }
                                else ->
                                    Column(
                                        Modifier.weight(1f)
                                            .verticalScroll(rememberScrollState())
                                            .padding(bottom = 16.dp)
                                    ) {
                                        PlayerPane(
                                            vm,
                                            expanded = true,
                                            showSurface = false,
                                            onQueue = {
                                                vm.loadQueue()
                                                page = "queue"
                                            },
                                            onSleep = { page = "sleep" },
                                            onDone = {
                                                close()
                                                exit()
                                            },
                                        )
                                    }
                            }
                        }
                    }
                }
            }
        }
    }
}
