package dev.ytui.android;

import androidx.media3.common.C;
import androidx.media3.common.Player;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Pure decisions behind PlaybackService's ticker, resume positions, preview restore and audio
 * attributes. Kept apart from the service so they can be tested without a player.
 */
final class PlaybackLifecycle {
  /** Player gain for listeners who never chose one: follow the system volume unchanged. */
  static final float DEFAULT_VOLUME = 1f;

  /** Ticker period while media is playing or buffering. */
  static final long TICK_MS = 200;

  /** How often the resume position is stored while playing. */
  static final long POSITION_SAVE_MS = 5000;

  /** The player gain to apply; a saved choice always wins over the default. */
  static float savedVolume(android.content.SharedPreferences prefs) {
    return prefs.getFloat("volume", DEFAULT_VOLUME);
  }

  /**
   * Whether the ticker has per-frame work: SponsorBlock skips, position saves and the progress
   * the UI shows all only change while the player wants to play.
   */
  static boolean tickActive(boolean playWhenReady, int playbackState) {
    return playWhenReady
        && (playbackState == Player.STATE_READY || playbackState == Player.STATE_BUFFERING);
  }

  /**
   * Delay before the next tick, or -1 to stop ticking. Without playback only a pending sleep
   * countdown needs a tick, once, at its deadline (so it cannot fire later after a resume).
   */
  static long tickDelay(boolean active, long sleepDeadline, long now) {
    if (active) return TICK_MS;
    if (sleepDeadline > 0) return Math.max(TICK_MS, sleepDeadline - now);
    return -1;
  }

  /**
   * Whether the current resume position is worth storing. Previews and live streams have none;
   * a position lookup in flight must not be overwritten by the start of the item; an ended item
   * was already reset to the beginning; zero never needs storing.
   */
  static boolean savesPosition(
      boolean historyEnabled,
      boolean preview,
      boolean live,
      boolean resumePending,
      int playbackState,
      long positionMs) {
    return historyEnabled
        && !preview
        && !live
        && !resumePending
        && playbackState != Player.STATE_ENDED
        && positionMs > 0;
  }

  /** Where an automatic or explicit resume seeks to, or -1 to stay at the start. */
  static long resumeTarget(long savedMs, boolean automatic) {
    if (savedMs < 0) return -1;
    return !automatic || savedMs >= PlaybackService.AUTO_RESUME_MIN_MS ? savedMs : -1;
  }

  /** Movies and music get different audio processing on some devices (ducking, effects). */
  static int contentType(boolean audioOnly) {
    return audioOnly ? C.AUDIO_CONTENT_TYPE_MUSIC : C.AUDIO_CONTENT_TYPE_MOVIE;
  }

  /**
   * Downloads of YouTube videos still have SponsorBlock segments (the lookup is a separate
   * network request whose failure is ignored). Other local files have no video ID to look up.
   */
  static boolean fetchesSponsorMarks(boolean offline, boolean local, String mediaUrl) {
    return (!offline && !local) || youtubeUrl(mediaUrl);
  }

  static boolean youtubeUrl(String url) {
    if (url == null) return false;
    try {
      String host = new java.net.URI(url.trim()).getHost();
      return host != null
          && java.util.Arrays.asList(
                  "youtube.com",
                  "www.youtube.com",
                  "m.youtube.com",
                  "music.youtube.com",
                  "youtu.be")
              .contains(host.toLowerCase(java.util.Locale.ROOT));
    } catch (java.net.URISyntaxException invalid) {
      return false;
    }
  }

  /**
   * The request that continues an item a preview interrupted. Marked as a reconnect, so play()
   * keeps the caller's pause intent instead of starting audibly, and it never looks up a saved
   * position, which would move playback away from where the preview interrupted it.
   */
  static JSONObject previewRestoreRequest(JSONObject interrupted) throws JSONException {
    return new JSONObject(interrupted.toString())
        .put("resume", false)
        .put("auto_resume", false)
        .put("reconnect", true);
  }

  private PlaybackLifecycle() {}
}
