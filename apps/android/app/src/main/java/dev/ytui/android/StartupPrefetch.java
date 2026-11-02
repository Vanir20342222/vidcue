package dev.ytui.android;

import android.net.Uri;
import androidx.media3.datasource.DataSpec;
import androidx.media3.exoplayer.dash.manifest.*;
import java.io.ByteArrayInputStream;
import java.nio.charset.StandardCharsets;
import java.util.*;
import org.json.JSONObject;

/** Warm the selected tracks' metadata before bulk prefixes, within the original byte budgets. */
@androidx.annotation.OptIn(markerClass = androidx.media3.common.util.UnstableApi.class)
final class StartupPrefetch {
  static final class Track {
    final String url;
    final int budget;
    final List<DataSpec> ranges = new ArrayList<>();
    int metadataBytes;

    Track(String url, int budget) {
      this.url = url;
      this.budget = budget;
    }
  }

  static List<Track> plan(JSONObject stream, boolean audio) {
    Map<String, Track> tracks = new LinkedHashMap<>();
    String primary = stream.optString("url");
    if (!web(primary)) throw new IllegalArgumentException("Preload requires an HTTP stream");
    tracks.put(primary, new Track(primary, audio ? 384 * 1024 : 1024 * 1024));
    String paired = stream.optString("audio_url");
    if (!audio && web(paired) && !paired.equals(primary))
      tracks.put(paired, new Track(paired, 192 * 1024));
    for (String field : new String[] {"dash_manifest", "audio_dash_manifest"}) {
      String xml = stream.optString(field);
      if (xml.isEmpty() || xml.length() > 65536) continue;
      try {
        DashManifest manifest =
            new DashManifestParser()
                .parse(
                    Uri.parse(PlaybackManifestDataSource.VIDEO),
                    new ByteArrayInputStream(xml.getBytes(StandardCharsets.UTF_8)));
        if (manifest.getPeriodCount() != 1) continue;
        for (AdaptationSet adaptation : manifest.getPeriod(0).adaptationSets) {
          for (Representation representation : adaptation.representations) {
            if (representation.baseUrls.isEmpty()) continue;
            String base = representation.baseUrls.get(0).url;
            Track track = tracks.get(base);
            if (track == null || !track.ranges.isEmpty()) continue;
            RangedUri init = representation.getInitializationUri();
            RangedUri index = representation.getIndexUri();
            RangedUri merged =
                init == null || index == null ? null : init.attemptMerge(index, base);
            if (merged != null) add(track, merged, base, stream);
            else {
              add(track, init, base, stream);
              add(track, index, base, stream);
            }
          }
        }
      } catch (Exception invalid) {
        // A speculative plan must not make valid foreground playback fail.
      }
    }
    for (Track track : tracks.values()) {
      track.ranges.add(
          new DataSpec.Builder()
              .setUri(track.url)
              .setKey(cacheKey(stream, track.url))
              .setLength(track.budget - track.metadataBytes)
              .build());
    }
    return new ArrayList<>(tracks.values());
  }

  private static void add(Track track, RangedUri range, String base, JSONObject stream) {
    if (range == null
        || range.start < 0
        || range.length <= 0
        || range.start > Long.MAX_VALUE - range.length
        || range.length > track.budget / 2 - track.metadataBytes
        || !range.resolveUri(base).equals(Uri.parse(track.url))) return;
    track.ranges.add(
        new DataSpec.Builder()
            .setUri(track.url)
            .setKey(cacheKey(stream, track.url))
            .setPosition(range.start)
            .setLength(range.length)
            .build());
    track.metadataBytes += (int) range.length;
  }

  static String cacheKey(JSONObject stream, String url) {
    String key =
        stream.optString(
            url.equals(stream.optString("audio_url")) ? "audio_cache_key" : "cache_key");
    return key.isEmpty() || key.equals("null") ? null : key;
  }

  private static boolean web(String url) {
    Uri uri = Uri.parse(url);
    return ("https".equals(uri.getScheme()) || "http".equals(uri.getScheme()))
        && uri.getHost() != null;
  }

  private StartupPrefetch() {}
}
