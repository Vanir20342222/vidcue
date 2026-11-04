package dev.ytui.android

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp

/**
 * A queued download's cover flies from where it was saved into the Downloads tab, instead of a
 * confirmation glyph popping up at the bottom of the screen. Positions are root coordinates.
 */
internal object DownloadFlight {
    data class Flight(val id: Long, val media: MediaCard, val from: Offset?)

    /** Centre of the Downloads destination in the navigation, once laid out. */
    var target by mutableStateOf<Offset?>(null)
    var flight by mutableStateOf<Flight?>(null)
        private set

    /** Increments when a cover lands, so the Downloads icon can answer with a bounce. */
    var landed by mutableLongStateOf(0L)
        private set

    private var ids = 0L

    fun launch(media: MediaCard, from: Offset?) {
        flight = Flight(++ids, media, from)
    }

    fun land(id: Long) {
        if (flight?.id == id) flight = null
        landed++
    }
}

/** Records the centre of this element, in root coordinates, into [into]. */
internal fun Modifier.trackCentre(into: (Offset) -> Unit): Modifier =
    onGloballyPositioned { into(it.boundsInRoot().center) }

/** Draws the flying cover over everything. Place once, filling the root. */
@Composable
internal fun DownloadFlightHost(thumbnails: Boolean) {
    val flight = DownloadFlight.flight ?: return
    val motion = LocalMotion.current
    Box(Modifier.fillMaxSize()) {
        key(flight.id) {
            val progress = remember { Animatable(0f) }
            LaunchedEffect(Unit) {
                if (motion.animate)
                    progress.animateTo(1f, tween(motion.millis(720), easing = FastOutSlowInEasing))
                DownloadFlight.land(flight.id)
            }
            if (motion.animate) FlyingCover(flight, { progress.value }, thumbnails)
        }
    }
}

@Composable
private fun FlyingCover(flight: DownloadFlight.Flight, progress: () -> Float, thumbnails: Boolean) {
    val density = LocalDensity.current
    run {
            var hostSize by remember { mutableStateOf(androidx.compose.ui.geometry.Size.Zero) }
            Box(
                Modifier.fillMaxSize().onGloballyPositioned {
                    hostSize = androidx.compose.ui.geometry.Size(it.size.width.toFloat(), it.size.height.toFloat())
                }
            ) {
                if (hostSize != androidx.compose.ui.geometry.Size.Zero) {
                val start = flight.from ?: Offset(hostSize.width / 2, hostSize.height * 0.55f)
                val end =
                    DownloadFlight.target
                        ?: Offset(hostSize.width * 0.875f, hostSize.height - with(density) { 48.dp.toPx() })
                // An arc: up and over, then down into the tab.
                val lift = with(density) { 140.dp.toPx() }
                val control = Offset((start.x + end.x) / 2, minOf(start.y, end.y) - lift)
                val cover = 64.dp
                val half = with(density) { cover.toPx() / 2 }
                Box(
                    Modifier.size(cover)
                        .graphicsLayer {
                            val t = progress()
                            val u = 1f - t
                            val x = u * u * start.x + 2 * u * t * control.x + t * t * end.x
                            val y = u * u * start.y + 2 * u * t * control.y + t * t * end.y
                            translationX = x - half
                            translationY = y - half
                            val scale = 1f - 0.7f * t
                            scaleX = scale
                            scaleY = scale
                            rotationZ = -12f * t
                            alpha = if (t < 0.85f) 1f else (1f - t) / 0.15f
                        }
                        .shadow(12.dp, RoundedCornerShape(14.dp), ambientColor = Color.Black, spotColor = Color.Black)
                        .testTag("download-flight")
                        .semantics {
                            contentDescription = "Download queued"
                            liveRegion = LiveRegionMode.Polite
                        }
                ) {
                    Artwork(flight.media, Modifier.fillMaxSize(), thumbnails, shape = RoundedCornerShape(14.dp))
                }
                }
            }
    }
}
