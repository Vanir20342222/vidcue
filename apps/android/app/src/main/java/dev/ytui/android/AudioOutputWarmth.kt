package dev.ytui.android

import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioTrack
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.currentStateAsState
import java.util.concurrent.atomic.AtomicLong
import kotlinx.coroutines.delay

/**
 * Keeps the music output path awake with an inaudible silent stream while someone is choosing
 * music, so a selected song starts at once.
 *
 * Measured on a Galaxy Tab S9 (speaker): a fresh music AudioTrack took 95-447 ms from play() to
 * its first advancing frame when the output was idle, and 27-46 ms while another stream kept the
 * output running. The idle cost is the device waking its output path (DSP, amplifiers), which no
 * player setting avoids. The silent stream holds no audio focus, never pauses other apps, and is
 * written as zero samples, so nothing is heard.
 */
internal object AudioOutputWarmth {
    private const val RATE = 48_000
    /** Each start gets a generation; a stream runs only while its generation is current. */
    private val generation = AtomicLong()
    @Volatile private var playing = 0L

    /** Starts the silent stream unless one runs or something (ours or another app) plays. */
    @Synchronized
    fun start(context: android.content.Context) {
        val audio = context.getSystemService(AudioManager::class.java) ?: return
        if (playing != 0L && playing == generation.get()) return
        if (audio.isMusicActive) return
        val mine = generation.incrementAndGet()
        playing = mine
        Thread({
                    val track =
                        runCatching {
                                AudioTrack.Builder()
                                    // The same usage and content type as music playback, so the
                                    // warmed output is the one a song is routed to.
                                    .setAudioAttributes(
                                        AudioAttributes.Builder()
                                            .setUsage(AudioAttributes.USAGE_MEDIA)
                                            .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                                            .build()
                                    )
                                    .setAudioFormat(
                                        AudioFormat.Builder()
                                            .setSampleRate(RATE)
                                            .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                                            .setChannelMask(AudioFormat.CHANNEL_OUT_STEREO)
                                            .build()
                                    )
                                    .setBufferSizeInBytes(
                                        AudioTrack.getMinBufferSize(
                                            RATE,
                                            AudioFormat.CHANNEL_OUT_STEREO,
                                            AudioFormat.ENCODING_PCM_16BIT,
                                        ) * 2
                                    )
                                    .setTransferMode(AudioTrack.MODE_STREAM)
                                    .build()
                            }
                            .getOrNull()
                    try {
                        if (track == null || track.state != AudioTrack.STATE_INITIALIZED) return@Thread
                        val silence = ByteArray(RATE / 50 * 4) // 20 ms of stereo 16-bit zeros
                        track.play()
                        if (BuildConfig.DEBUG) android.util.Log.d("AudioWarmth", "Output warm")
                        // A blocking write paces the loop at real time; it uses no CPU between.
                        while (generation.get() == mine && track.write(silence, 0, silence.size) >= 0) Unit
                    } catch (_: Exception) {
                        // Warmth is only an optimisation; any audio error just ends it.
                    } finally {
                        runCatching { track?.pause() }
                        runCatching { track?.flush() }
                        runCatching { track?.release() }
                        synchronized(this) { if (playing == mine) playing = 0L }
                    }
                }, "vidcue-audio-warmth")
                .apply {
                    isDaemon = true
                    start()
                }
    }

    @Synchronized
    fun stop() {
        generation.incrementAndGet()
        playing = 0L
    }
}

/**
 * Warms the music output while [wanted] holds and the activity is resumed, for at most
 * [limitMs] at a time so an open, untouched Music page does not keep the output awake for long.
 */
@Composable
internal fun WarmAudioOutputWhile(wanted: Boolean, limitMs: Long = 120_000) {
    val context = LocalContext.current.applicationContext
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    val state by lifecycle.currentStateAsState()
    val resumed = state.isAtLeast(Lifecycle.State.RESUMED)
    DisposableEffect(lifecycle) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_PAUSE) AudioOutputWarmth.stop()
        }
        lifecycle.addObserver(observer)
        onDispose {
            lifecycle.removeObserver(observer)
            AudioOutputWarmth.stop()
        }
    }
    LaunchedEffect(wanted, resumed) {
        if (!wanted || !resumed) {
            AudioOutputWarmth.stop()
            return@LaunchedEffect
        }
        // Retry while wanted: right after our own playback stops the system can still report
        // music as active for a moment, and start() declines then.
        val until = android.os.SystemClock.elapsedRealtime() + limitMs
        while (android.os.SystemClock.elapsedRealtime() < until) {
            AudioOutputWarmth.start(context)
            delay(1_000)
        }
        AudioOutputWarmth.stop()
    }
}
