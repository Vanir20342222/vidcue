package dev.ytui.android;

import static org.junit.Assert.*;

import android.content.Context;
import android.content.SharedPreferences;
import java.io.File;
import org.json.*;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;

@RunWith(RobolectricTestRunner.class)
public final class PrepareWorkerTest {
  @Test
  public void purgeRemovesOnlyThatJobsCheckpointAndProgress() throws Exception {
    Context context = org.robolectric.RuntimeEnvironment.getApplication();
    SharedPreferences progress =
        context.getSharedPreferences("preparations-progress", Context.MODE_PRIVATE);
    progress.edit().putInt("a", 3).putInt("a-failed", 1).putInt("b", 2).commit();
    File a = PrepareWorker.checkpoint(context, "a"), b = PrepareWorker.checkpoint(context, "b");
    assertTrue(a.createNewFile());
    assertTrue(b.createNewFile());
    PrepareWorker.purge(context, "a");
    assertFalse(a.exists());
    assertFalse(progress.contains("a"));
    assertFalse(progress.contains("a-failed"));
    assertTrue(b.exists());
    assertEquals(2, progress.getInt("b", 0));
    // Purging a job with nothing stored is harmless.
    PrepareWorker.purge(context, "missing");
  }

  @Test
  public void downloadedPlaylistKeepsOrderAndLinksToTheExactAudioDownloads() throws Exception {
    JSONObject request = new JSONObject()
        .put("request", new JSONObject().put("Collection", "VLfixture"))
        .put("title", "Road trip").put("audio", true).put("quality", "720");
    JSONObject first = new JSONObject().put("kind", "song").put("title", "First")
        .put("url", "https://music.youtube.com/watch?v=first");
    JSONObject second = new JSONObject().put("kind", "song").put("title", "Second")
        .put("url", "https://music.youtube.com/watch?v=second");
    JSONArray items = new JSONArray().put(first).put(second).put(first)
        .put(new JSONObject().put("kind", "playlist"))
        .put(new JSONObject().put("kind", "song").put("live", true));
    JSONObject playlist = PrepareWorker.offlinePlaylist(request, items);
    assertEquals("Road trip", playlist.getString("title"));
    JSONArray members = playlist.getJSONArray("items");
    assertEquals(2, members.length());
    assertEquals("First", members.getJSONObject(0).getString("title"));
    assertEquals("Second", members.getJSONObject(1).getString("title"));
    assertEquals(OfflineStore.id(new JSONObject().put("media", first).put("audio", true)),
        members.getJSONObject(0).getString("offline_id"));
    assertFalse(first.has("offline_id"));
    assertEquals(playlist.toString(), PrepareWorker.offlinePlaylist(request, items).toString());
    // A repeat download updates the same playlist even if the title/format changes.
    request.put("title", "Renamed").put("audio", false);
    assertEquals(playlist.getString("id"),
        PrepareWorker.offlinePlaylist(request, items).getString("id"));
    assertNotEquals(members.getJSONObject(0).getString("offline_id"),
        PrepareWorker.offlinePlaylist(request, items).getJSONArray("items")
            .getJSONObject(0).getString("offline_id"));
  }
}
