package dev.ytui.android

import androidx.media3.common.C
import androidx.media3.common.Player
import androidx.media3.common.Timeline

/**
 * Where a live stream's playback sits relative to its live edge (M9).
 *
 * Media3 reports a live stream's position and duration relative to the moving DVR window.
 * Shown as plain time this read like a finished video ("1:00:28 / 1:00:00"). The player
 * instead shows LIVE at the edge, how far behind it playback is, and a tap that returns there.
 * Live is detected from the loaded timeline, so a stream the catalogue did not mark live
 * still gets live controls.
 */
internal object LivePosition {
    /**
     * Playback this close to the window's default (live) position counts as live. Media3's
     * live speed control holds the offset within about a second of its target; one 5 s rewind
     * must already read as behind.
     */
    const val EDGE_TOLERANCE_MS = 4_000L

    /** How far [positionMs] trails the live edge, or 0 when the edge is unknown. */
    @JvmStatic
    fun behindMs(edgeMs: Long, positionMs: Long): Long =
        if (edgeMs == C.TIME_UNSET || edgeMs < 0 || positionMs < 0) 0L
        else (edgeMs - positionMs).coerceAtLeast(0L)

    @JvmStatic fun atEdge(behindMs: Long): Boolean = behindMs <= EDGE_TOLERANCE_MS

    /**
     * A live window's position can run past the window duration between playlist refreshes;
     * keep it on the bar instead of printing a time beyond the end.
     */
    @JvmStatic
    fun clamp(positionMs: Long, durationMs: Long): Long =
        if (durationMs == C.TIME_UNSET || durationMs <= 0) positionMs.coerceAtLeast(0L)
        else positionMs.coerceIn(0L, durationMs)

    /** "−0:45" or "−1:02:03": how far behind live, for the time pill. Empty at the edge. */
    @JvmStatic
    fun behindLabel(behindMs: Long): String {
        if (atEdge(behindMs)) return ""
        val total = behindMs / 1000
        val hours = total / 3600
        val minutes = (total % 3600) / 60
        val seconds = total % 60
        return if (hours > 0) "\u2212%d:%02d:%02d".format(java.util.Locale.ROOT, hours, minutes, seconds)
        else "\u2212%d:%02d".format(java.util.Locale.ROOT, minutes, seconds)
    }

    /** True while the player's current item is a live stream (not a finished recording). */
    @JvmStatic
    fun live(player: Player?): Boolean =
        player != null && !player.currentTimeline.isEmpty && player.isCurrentMediaItemLive

    /** The live edge in the same window-relative frame as Player.getCurrentPosition. */
    @JvmStatic
    fun edgeMs(player: Player): Long {
        val timeline = player.currentTimeline
        if (timeline.isEmpty) return C.TIME_UNSET
        val index = player.currentMediaItemIndex
        if (index < 0 || index >= timeline.windowCount) return C.TIME_UNSET
        val window = timeline.getWindow(index, Timeline.Window())
        val edge = window.defaultPositionMs
        return if (edge != C.TIME_UNSET && edge > 0) edge else player.duration
    }

    @JvmStatic
    fun behindMs(player: Player?): Long =
        if (!live(player)) 0L else behindMs(edgeMs(player!!), player.currentPosition)

    /** Back to the live edge; resumes playback too, as a "go live" control does. */
    @JvmStatic
    fun goLive(player: Player?) {
        if (!live(player)) return
        player!!.seekToDefaultPosition()
        player.playWhenReady = true
    }
}
