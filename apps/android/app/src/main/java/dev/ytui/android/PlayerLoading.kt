package dev.ytui.android

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay

/** A stall shorter than this is normal streaming jitter and shows no spinner. */
private const val STALL_SPINNER_DELAY_MS = 400L

/** True once buffering has lasted a moment, so brief rebuffers do not flicker a spinner. */
@Composable
internal fun rememberStalled(buffering: Boolean): Boolean {
    var stalled by remember { mutableStateOf(false) }
    LaunchedEffect(buffering) {
        if (buffering) {
            delay(STALL_SPINNER_DELAY_MS)
            stalled = true
        } else {
            stalled = false
        }
    }
    return stalled
}

/** Loading indicator on a soft scrim, so it reads over cover art and video alike. */
@Composable
internal fun LoadingSpinner(modifier: Modifier = Modifier) {
    Box(
        modifier
            .size(56.dp)
            .clip(CircleShape)
            .background(Color.Black.copy(alpha = 0.45f))
            .testTag("player-loading")
            .semantics { contentDescription = "Loading" },
        contentAlignment = Alignment.Center,
    ) {
        MediaSpinner(color = Color.White)
    }
}
