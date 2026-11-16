package dev.ytui.android;

import java.util.ArrayDeque;
import java.util.concurrent.atomic.AtomicLong;

/** Local, bounded, identifier-free tap-to-output measurements. All times are monotonic. */
final class StartupTimings {
  static final long TARGET_MS = 500;
  static final long AUDIO_TARGET_MS = 100;
  static final String[] PHASES = {
    "selection",
    "offline",
    "offline_lookup",
    "rpc_queue",
    "engine",
    "flight_wait",
    "slot",
    "worker",
    "client",
    "extract",
    "visitor",
    "player",
    "manifest",
    "stream",
    "decoder",
    "ready",
    "net_response",
    "net_byte",
    "codec",
    "audio_codec",
    "codec_init",
    "audio_codec_init",
    "view_inflate",
    "service_setup"
  };
  private static final AtomicLong ids = new AtomicLong();
  private static final ArrayDeque<Sample> samples = new ArrayDeque<>();

  private static final class Sample {
    long id, tap, resolved = -1, handoff = -1, output = -1, playing = -1;
    boolean prepared, video;
    String route = "unknown", cache = "unknown", disposition = "pending/cancelled";
    String quality = "unknown", transport = "unknown";
    long ended = -1;
    long audioTrack = -1;
    int byteCache = -1, playerCache = -1, sourceCache = -1;
    long[] stages = new long[PHASES.length];

    Sample(long id, long tap, boolean video) {
      this.id = id;
      this.tap = tap;
      this.video = video;
      java.util.Arrays.fill(stages, -1);
    }
  }

  static synchronized long begin(long now, boolean video) {
    long id = ids.incrementAndGet();
    samples.addLast(new Sample(id, now, video));
    while (samples.size() > 20) samples.removeFirst();
    return id;
  }

  static synchronized long latestToken() {
    Sample sample = samples.peekLast();
    return sample == null ? 0 : sample.id;
  }

  private static Sample find(long id) {
    for (Sample sample : samples) if (sample.id == id) return sample;
    return null;
  }

  static synchronized void resolved(long id, long now, boolean prepared) {
    Sample sample = find(id);
    if (sample != null && now >= sample.tap && sample.resolved < 0) {
      sample.resolved = now - sample.tap;
      sample.prepared = prepared;
    }
  }

  static synchronized void route(long id, String route) {
    Sample sample = find(id);
    if (sample != null)
      sample.route =
          java.util.Arrays.asList("direct", "full", "offline", "direct-media").contains(route)
              ? route
              : "unknown";
  }

  static synchronized void presentation(long id, String quality, String transport) {
    Sample sample = find(id);
    if (sample == null) return;
    sample.quality =
        java.util.Arrays.asList("360", "480", "720", "1080", "best").contains(quality)
            ? quality
            : "unknown";
    sample.transport =
        java.util.Arrays.asList(
                    "hls-chunkless", "hls", "dash", "dash-indexed", "progressive", "offline")
                .contains(transport)
            ? transport
            : "unknown";
  }

  static synchronized void stage(long id, String phase, long milliseconds) {
    Sample sample = find(id);
    int index = java.util.Arrays.asList(PHASES).indexOf(phase);
    if (sample != null
        && index >= 0
        && sample.stages[index] < 0
        && milliseconds >= 0
        && milliseconds <= 3600000) sample.stages[index] = milliseconds;
  }

  static synchronized void cacheState(long id, String source) {
    Sample sample = find(id);
    if (sample == null || !java.util.Arrays.asList("ready", "joined", "miss").contains(source))
      return;
    sample.cache = source;
    // Joining a still-running preload is not a stream prepared before the tap.
    sample.prepared = "ready".equals(source);
  }

  static synchronized void sourceCache(long id, boolean reused) {
    Sample sample = find(id);
    if (sample != null) sample.sourceCache = reused ? 1 : 0;
  }

  static synchronized void playerCache(long id, boolean ready) {
    Sample sample = find(id);
    if (sample != null && sample.playerCache < 0) sample.playerCache = ready ? 1 : 0;
  }

  static synchronized void byteCache(long id, boolean ready) {
    Sample sample = find(id);
    if (sample != null && sample.byteCache < 0) sample.byteCache = ready ? 1 : 0;
  }

  static synchronized void ended(long id, String disposition, long now) {
    Sample sample = find(id);
    if (sample != null
        && (sample.output < 0 || (sample.video && sample.playing < 0))
        && sample.ended < 0
        && now >= sample.tap
        && java.util.Arrays.asList("failed", "cancelled").contains(disposition)) {
      sample.disposition = disposition;
      sample.ended = now - sample.tap;
    }
  }

  static synchronized void mediaType(long id, boolean video) {
    Sample sample = find(id);
    if (sample != null && sample.handoff < 0) sample.video = video;
  }

  static synchronized void handoff(long id, long now) {
    Sample sample = find(id);
    if (sample != null && now >= sample.tap && sample.handoff < 0)
      sample.handoff = now - sample.tap;
  }

  static synchronized void output(long id, long now, boolean video) {
    Sample sample = find(id);
    if (sample != null
        && sample.video == video
        && sample.handoff >= 0
        && sample.ended < 0
        && now >= sample.tap
        && sample.output < 0) {
      sample.output = now - sample.tap;
      if (!video) sample.playing = sample.output;
    }
  }

  static synchronized void started(long id, long now) {
    Sample sample = find(id);
    if (sample != null
        && sample.ended < 0
        && sample.playing < 0
        && sample.handoff >= 0
        && now >= sample.tap) sample.playing = now - sample.tap;
  }

  static synchronized long usable(long id) {
    Sample sample = find(id);
    return sample == null || sample.output < 0 || sample.playing < 0
        ? -1
        : Math.max(sample.output, sample.playing);
  }

  /** AudioTrack position advancement is stronger evidence than Player.isPlaying alone. */
  static synchronized void audioOutput(long id, long now) {
    Sample sample = find(id);
    if (sample != null && !sample.video && sample.handoff >= 0 && sample.ended < 0
        && sample.audioTrack < 0 && now >= sample.tap) sample.audioTrack = now - sample.tap;
  }

  static synchronized long audioUsable(long id) {
    Sample sample = find(id);
    long active = usable(id);
    return sample == null || sample.video || sample.audioTrack < 0 || active < 0
        ? -1 : Math.max(active, sample.audioTrack);
  }

  static synchronized long elapsed(long id, long now) {
    Sample sample = find(id);
    return sample == null || now < sample.tap ? -1 : now - sample.tap;
  }

  static synchronized long total(long id) {
    Sample sample = find(id);
    return sample == null ? -1 : sample.output;
  }

  static synchronized String report() {
    StringBuilder text =
        new StringBuilder("Vidcue startup timings (milliseconds)\n")
            .append("Target: video tap to visible frame and active playback < 500 ms\n")
            .append("Target: audio tap to active playback < 100 ms (not acoustic output).\n")
            .append(
                "Resolve includes offline lookup and queue selection. Pending attempts are"
                    + " included.\n");
    for (Sample sample : samples) {
      text.append(sample.video ? "Video" : "Audio")
          .append(sample.prepared ? " prepared" : " cold")
          .append(" route=")
          .append(sample.route)
          .append(": resolve=")
          .append(sample.resolved)
          .append(", handoff=")
          .append(sample.handoff)
          .append(sample.video ? ", visible_frame=" : ", playing=")
          .append(sample.output)
          .append(
              sample.output < 0
                  ? " " + sample.disposition
                  : sample.output >= (sample.video ? TARGET_MS : AUDIO_TARGET_MS)
                      ? " OVER TARGET" : "")
          .append(" cache=")
          .append(sample.cache);
      if (sample.ended >= 0) text.append(" ended_ms=").append(sample.ended);
      for (int i = 0; i < PHASES.length; i++)
        if (sample.stages[i] >= 0)
          text.append(" ").append(PHASES[i]).append("_ms=").append(sample.stages[i]);
      text.append(" quality=")
          .append(sample.quality)
          .append(" transport=")
          .append(sample.transport);
      if (sample.sourceCache >= 0)
        text.append(" source_preloaded_reused=").append(sample.sourceCache == 1);
      if (sample.playerCache >= 0)
        text.append(" player_at_tap=").append(sample.playerCache == 1 ? "ready" : "cold");
      if (sample.byteCache >= 0)
        text.append(" warmup_bytes_at_tap=").append(sample.byteCache == 1 ? "ready" : "not_ready");
      text.append(" started=")
          .append(sample.playing)
          .append(" usable_start=")
          .append(usable(sample.id));
      if (!sample.video)
        text.append(" audio_track_ms=").append(sample.audioTrack)
            .append(" audio_usable_ms=").append(audioUsable(sample.id));
      if (!sample.video && audioUsable(sample.id) >= AUDIO_TARGET_MS)
        text.append(" AUDIO OUTPUT OVER TARGET");
      if (usable(sample.id) >= (sample.video ? TARGET_MS : AUDIO_TARGET_MS))
        text.append(" USABLE OVER TARGET");
      else if (sample.video && sample.output >= 0 && sample.playing < 0)
        text.append(" ").append(sample.ended >= 0 ? sample.disposition : "waiting_for_playback");
      text.append('\n');
    }
    return text.toString();
  }

  private StartupTimings() {}
}
