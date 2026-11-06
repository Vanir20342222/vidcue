package dev.ytui.android;

import androidx.media3.datasource.HttpDataSource;
import androidx.media3.exoplayer.offline.Download;
import androidx.media3.exoplayer.scheduler.Requirements;
import java.util.*;
import org.json.JSONObject;

/** Pure download decisions (stream refresh, free space, network, notification), kept testable. */
@androidx.annotation.OptIn(markerClass = androidx.media3.common.util.UnstableApi.class)
final class DownloadPolicy {
  /** Space required when the stream does not state its size. */
  static final long UNKNOWN_SIZE_FLOOR = 200L * 1024 * 1024;

  /** Headroom kept free beyond a known download size. */
  static final long RESERVE_BYTES = 64L * 1024 * 1024;

  /** Request-data flag on a download that was already re-resolved automatically once. */
  static final String AUTO_REFRESHED = "auto_refreshed";

  static final String AUDIO_SUFFIX = "-audio";

  /** The user-visible download id for a main or paired audio part. */
  static String baseId(String id) {
    return id.endsWith(AUDIO_SUFFIX) ? id.substring(0, id.length() - AUDIO_SUFFIX.length()) : id;
  }

  /** HTTP status in [error]'s cause chain, or -1. */
  static int responseCode(Throwable error) {
    for (int depth = 0; error != null && depth < 8; depth++, error = error.getCause())
      if (error instanceof HttpDataSource.InvalidResponseCodeException)
        return ((HttpDataSource.InvalidResponseCodeException) error).responseCode;
    return -1;
  }

  /** Signed CDN URLs answer 403 (or 410) once they expire; a fresh resolve fixes that. */
  static boolean expiredStream(Throwable error) {
    int code = responseCode(error);
    return code == 403 || code == 410;
  }

  /**
   * One automatic re-resolve per user-started stream: a request produced by an automatic refresh
   * is never refreshed again, so a stream that keeps failing cannot loop.
   */
  static boolean autoRefreshAllowed(JSONObject requestData) {
    return requestData != null && !requestData.optBoolean(AUTO_REFRESHED);
  }

  /** Bytes the selected stream(s) still need, from clen/filesize, or the unknown-size floor. */
  static long requiredBytes(JSONObject stream, long alreadyDownloaded) {
    long video = size(stream, "url", "filesize");
    String audioUrl = stream.optString("audio_url", "");
    boolean paired = !audioUrl.isEmpty() && !"null".equals(audioUrl);
    long audio = paired ? size(stream, "audio_url", "audio_filesize") : 0;
    if (video <= 0 || audio < 0) return UNKNOWN_SIZE_FLOOR;
    long remaining = Math.max(0, video + audio - Math.max(0, alreadyDownloaded));
    return remaining + RESERVE_BYTES;
  }

  /** Declared size of one part, or -1 when unknown. */
  private static long size(JSONObject stream, String urlKey, String sizeKey) {
    long declared = stream.optLong(sizeKey, -1);
    if (declared > 0) return declared;
    if ("filesize".equals(sizeKey)) {
      declared = stream.optLong("filesize_approx", -1);
      if (declared > 0) return declared;
    }
    String url = stream.optString(urlKey, "");
    int at = url.indexOf('?');
    if (at < 0) return -1;
    for (String pair : url.substring(at + 1).split("&")) {
      if (!pair.startsWith("clen=")) continue;
      try {
        long value = Long.parseLong(pair.substring(5));
        return value > 0 ? value : -1;
      } catch (NumberFormatException ignored) {
        return -1;
      }
    }
    return -1;
  }

  static boolean hasRoom(long usableBytes, long requiredBytes) {
    return usableBytes >= requiredBytes;
  }

  static String spaceError(long usableBytes, long requiredBytes) {
    return "Not enough free space to download: needs about "
        + megabytes(requiredBytes)
        + " MB, "
        + megabytes(usableBytes)
        + " MB free";
  }

  private static long megabytes(long bytes) {
    return Math.max(0, (bytes + 512 * 1024) / (1024 * 1024));
  }

  /** DownloadManager network requirement for the Wi-Fi-only setting. */
  static int networkRequirement(boolean unmeteredOnly) {
    return unmeteredOnly ? Requirements.NETWORK_UNMETERED : Requirements.NETWORK;
  }

  /** One download part as the notification sees it. */
  static final class Part {
    final String id;
    final int state;
    final long bytes, length;
    final float percent;

    Part(String id, int state, long bytes, long length, float percent) {
      this.id = id;
      this.state = state;
      this.bytes = bytes;
      this.length = length;
      this.percent = percent;
    }
  }

  /** Aggregate notification state over the current downloads. */
  static final class Summary {
    /** User-visible transfers; a video and its paired audio part count once. */
    final int items;

    final int downloading;

    /** 0–100, or -1 when no running part reports progress. */
    final int percent;

    Summary(int items, int downloading, int percent) {
      this.items = items;
      this.downloading = downloading;
      this.percent = percent;
    }
  }

  static Summary summarize(List<Part> parts) {
    Set<String> items = new LinkedHashSet<>(), active = new HashSet<>();
    long bytes = 0, length = 0;
    double percentSum = 0;
    int percentCount = 0;
    boolean lengthsKnown = true;
    for (Part part : parts) {
      if (part.state == Download.STATE_COMPLETED
          || part.state == Download.STATE_FAILED
          || part.state == Download.STATE_REMOVING) continue;
      String id = baseId(part.id);
      items.add(id);
      if (part.state == Download.STATE_DOWNLOADING) active.add(id);
      if (part.length > 0) {
        bytes += Math.max(0, Math.min(part.bytes, part.length));
        length += part.length;
      } else lengthsKnown = false;
      if (part.percent >= 0) {
        percentSum += Math.min(100f, part.percent);
        percentCount++;
      }
    }
    int percent = -1;
    if (!active.isEmpty()) {
      if (lengthsKnown && length > 0) percent = (int) Math.min(100, bytes * 100 / length);
      else if (percentCount > 0) percent = (int) Math.round(percentSum / percentCount);
    }
    return new Summary(items.size(), active.size(), percent);
  }

  /** Notification body for [summary] given the DownloadService's unmet requirements. */
  static String notificationText(Summary summary, int notMetRequirements) {
    if (summary.items == 0) return "Finishing downloads";
    String count = summary.items == 1 ? "1 download" : summary.items + " downloads";
    if ((notMetRequirements & Requirements.NETWORK_UNMETERED) != 0)
      return count + " · waiting for Wi-Fi";
    if ((notMetRequirements & Requirements.NETWORK) != 0)
      return count + " · waiting for a network";
    if (notMetRequirements != 0) return count + " · waiting";
    if (summary.downloading == 0) return count + " queued";
    return summary.percent >= 0 ? count + " · " + summary.percent + "%" : count;
  }

  private DownloadPolicy() {}
}
