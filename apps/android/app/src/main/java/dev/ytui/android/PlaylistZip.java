package dev.ytui.android;

import android.net.Uri;
import java.io.IOException;
import java.io.OutputStream;
import java.util.ArrayList;
import java.util.List;
import java.util.function.BooleanSupplier;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;
import org.json.JSONArray;
import org.json.JSONObject;

/** Packs an offline playlist's completed media into one zip, in playlist order. */
final class PlaylistZip {
  private PlaylistZip() {}

  /** An item with a complete offline copy, with its position in the playlist. */
  private static final class Entry {
    final int position;
    final OfflineExport.Plan plan;

    Entry(int position, OfflineExport.Plan plan) {
      this.position = position;
      this.plan = plan;
    }
  }

  /** The stored offline playlist with [id]. */
  static JSONObject find(String id) throws Exception {
    JSONArray playlists =
        new JSONArray(
            NativeBridge.request(new JSONObject().put("op", "offline_playlists").toString()));
    for (int i = 0; i < playlists.length(); i++) {
      JSONObject playlist = playlists.getJSONObject(i);
      if (id.equals(playlist.optString("id"))) return playlist;
    }
    throw new IllegalStateException("Offline playlist no longer exists");
  }

  /**
   * Writes each item that has a complete offline copy to [destination], named with its playlist
   * position ("001 - Title.m4a") so the order survives on other devices. Returns how many items
   * were left out for lacking such a copy. Fails before creating the zip when none qualify.
   */
  static int write(JSONObject playlist, Uri destination, BooleanSupplier cancelled)
      throws Exception {
    JSONArray items = playlist.optJSONArray("items");
    List<Entry> entries = new ArrayList<>();
    int skipped = 0;
    for (int i = 0; items != null && i < items.length(); i++) {
      try {
        String id = items.getJSONObject(i).optString("offline_id");
        entries.add(new Entry(i, OfflineExport.inspect(id)));
      } catch (Exception unavailable) {
        skipped++;
      }
    }
    if (entries.isEmpty())
      throw new UnsupportedOperationException(
          "This playlist has no completed offline media to export");
    OutputStream output =
        MediaBridge.context.getContentResolver().openOutputStream(destination, "w");
    if (output == null) throw new IOException("No output stream");
    try (ZipOutputStream zip = new ZipOutputStream(output)) {
      for (Entry entry : entries) {
        String name =
            PlaybackPolicy.exportName(entry.plan.title, entry.plan.extension, entry.position);
        zip.putNextEntry(new ZipEntry(name));
        OfflineExport.writeTo(entry.plan, zip, cancelled);
        zip.closeEntry();
      }
    }
    return skipped;
  }
}
