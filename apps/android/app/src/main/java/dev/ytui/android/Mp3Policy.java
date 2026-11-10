package dev.ytui.android;

import java.util.ArrayList;
import java.util.List;

/** Pure decisions for MP3 conversion: what to convert, what to call it, and whether it is sound. */
final class Mp3Policy {
  private Mp3Policy() {}

  /** An offline item the backfill may convert. */
  static final class Candidate {
    final String id;
    final boolean audio;
    /** Name or MIME of the file already saved to a folder, or null when there is none. */
    final String savedAs;
    /** True after this item has been converted once. */
    final boolean converted;

    Candidate(String id, boolean audio, String savedAs, boolean converted) {
      this.id = id;
      this.audio = audio;
      this.savedAs = savedAs;
      this.converted = converted;
    }
  }

  static boolean isMp3(String nameOrMime) {
    if (nameOrMime == null) return false;
    String value = nameOrMime.toLowerCase(java.util.Locale.ROOT);
    return value.equals("audio/mpeg")
        || value.equals("audio/mp3")
        || value.endsWith(".mp3")
        || value.endsWith("%2emp3");
  }

  /** Whether one item still needs converting. Video, finished and MP3 items never do. */
  static boolean needsConversion(Candidate item) {
    return item.audio
        && !item.id.endsWith("-audio")
        && !item.converted
        && !isMp3(item.savedAs);
  }

  /** The ids to convert, in the order given, without repeats. */
  static List<String> backfill(List<Candidate> items) {
    List<String> ids = new ArrayList<>();
    for (Candidate item : items) if (needsConversion(item) && !ids.contains(item.id)) ids.add(item.id);
    return ids;
  }

  /** The automatic conversion applies to a finished download only when the setting is on. */
  static boolean automatic(boolean settingOn, boolean audio) {
    return settingOn && audio;
  }

  /** File name of the converted copy, numbered when it belongs to a playlist. */
  static String fileName(String title, int playlistPosition) {
    return playlistPosition < 0
        ? PlaybackPolicy.exportName(title, "mp3")
        : PlaybackPolicy.exportName(title, "mp3", playlistPosition);
  }

  /**
   * Sample rate and channel count of the first MPEG audio frame in [head], skipping an ID3v2 tag,
   * or null when no valid MPEG-1/2/2.5 Layer III frame starts the data.
   */
  static int[] firstFrame(byte[] head, int length) {
    int at = 0;
    if (length >= 10 && head[0] == 'I' && head[1] == 'D' && head[2] == '3') {
      at = 10 + ((head[6] & 0x7f) << 21 | (head[7] & 0x7f) << 14 | (head[8] & 0x7f) << 7 | (head[9] & 0x7f));
    }
    if (at < 0 || at + 4 > length) return null;
    int h = (head[at] & 255) << 24 | (head[at + 1] & 255) << 16 | (head[at + 2] & 255) << 8 | (head[at + 3] & 255);
    if ((h >>> 21) != 0x7ff) return null; // frame sync
    int version = (h >>> 19) & 3; // 0 = 2.5, 1 = reserved, 2 = MPEG-2, 3 = MPEG-1
    int layer = (h >>> 17) & 3; // 1 = Layer III
    int bitrate = (h >>> 12) & 15, rate = (h >>> 10) & 3, mode = (h >>> 6) & 3;
    if (version == 1 || layer != 1 || bitrate == 0 || bitrate == 15 || rate == 3) return null;
    int[] rates = {44100, 48000, 32000};
    int sampleRate = rates[rate] >> (version == 3 ? 0 : version == 2 ? 1 : 2);
    return new int[] {sampleRate, mode == 3 ? 1 : 2};
  }

  /**
   * Whether a finished MP3 of [bytes] is a believable encoding of [frames] sample frames at
   * [sampleRate] Hz made at [kbps] (constant bitrate): within 10% plus one second of audio.
   */
  static boolean plausibleSize(long bytes, long frames, int sampleRate, int kbps) {
    if (bytes <= 0 || frames <= 0 || sampleRate <= 0) return false;
    double seconds = (double) frames / sampleRate;
    double expected = seconds * kbps * 1000 / 8;
    double slack = expected * 0.10 + kbps * 1000 / 8.0;
    return Math.abs(bytes - expected) <= slack;
  }
}
