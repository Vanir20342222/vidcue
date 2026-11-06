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
    progress.edit().putInt("a", 3).putInt("a-failed", 1).putString("a-retry", "1").putInt("b", 2).commit();
    File a = PrepareWorker.checkpoint(context, "a"), b = PrepareWorker.checkpoint(context, "b");
    assertTrue(a.createNewFile());
    assertTrue(b.createNewFile());
    File retry = PrepareWorker.checkpoint(context, "a-retry");
    assertTrue(retry.createNewFile());
    PrepareWorker.purge(context, "a");
    assertFalse(a.exists());
    assertFalse(retry.exists());
    assertFalse(progress.contains("a-retry"));
    assertFalse(progress.contains("a"));
    assertFalse(progress.contains("a-failed"));
    assertTrue(b.exists());
    assertEquals(2, progress.getInt("b", 0));
    // Purging a job with nothing stored is harmless.
    PrepareWorker.purge(context, "missing");
  }

  @Test
  public void completedPassRetainsOnlyFailuresWithoutReplacingTheFullOfflinePlaylist() throws Exception {
    Context context = org.robolectric.RuntimeEnvironment.getApplication();
    SharedPreferences progress =
        context.getSharedPreferences("preparations-progress", Context.MODE_PRIVATE);
    JSONObject request = new JSONObject().put("request", new JSONObject().put("Collection", "VLtest"))
        .put("create_offline_playlist", true).put("audio", true).put("quality", "720");
    JSONArray items = new JSONArray();
    for (int i = 0; i < 4; i++) items.put(new JSONObject().put("url", "fixture:" + i));
    File checkpoint = PrepareWorker.checkpoint(context, "completed");
    PrepareWorker.writeCheckpoint(checkpoint, items);
    progress.edit().putInt("completed", 4).putInt("completed-failed", 2)
        .putString("completed-retry", "1,3").putInt("other", 9).commit();

    JSONObject next = PrepareWorker.completedRequest(context, "completed", request, items, "1,3", 2);
    assertTrue(next.getBoolean("retry_failed"));
    assertFalse(next.getBoolean("create_offline_playlist"));
    assertTrue(request.getBoolean("create_offline_playlist"));
    assertTrue(next.getBoolean("audio"));
    assertEquals("720", next.getString("quality"));
    assertEquals(request.getJSONObject("request").toString(), next.getJSONObject("request").toString());
    assertFalse(checkpoint.exists());
    JSONArray retry = PrepareWorker.readCheckpoint(PrepareWorker.checkpoint(context, "completed-retry"));
    assertEquals(2, retry.length());
    assertEquals("fixture:1", retry.getJSONObject(0).getString("url"));
    assertEquals("fixture:3", retry.getJSONObject(1).getString("url"));
    for (String key : PreparationPolicy.progressKeys("completed")) assertFalse(progress.contains(key));
    assertEquals(9, progress.getInt("other", 0));

    // A second failed pass narrows the same job to the one remaining entry.
    JSONObject third = PrepareWorker.completedRequest(context, "completed", next, retry, "1", 1);
    JSONArray remaining = PrepareWorker.readCheckpoint(PrepareWorker.checkpoint(context, "completed-retry"));
    assertEquals(1, remaining.length());
    assertEquals("fixture:3", remaining.getJSONObject(0).getString("url"));
    // A successful retry drops both checkpoints and the old cursor without touching another job.
    PrepareWorker.completedRequest(context, "completed", third, remaining, "", 0);
    assertFalse(PrepareWorker.checkpoint(context, "completed-retry").exists());
    assertEquals(9, progress.getInt("other", 0));
  }

  @Test
  public void largeRetryListsStayOnDiskAndInterruptedRetryKeepsItsCursor() throws Exception {
    Context context = org.robolectric.RuntimeEnvironment.getApplication();
    JSONArray items = new JSONArray();
    String failures = "";
    for (int i = 0; i < 100; i++) {
      items.put(new JSONObject().put("url", "fixture:" + i).put("title", "x".repeat(200)));
      failures = PreparationPolicy.withFailure(failures, i);
    }
    JSONObject next = PrepareWorker.completedRequest(context, "large", new JSONObject(), items, failures, 100);
    assertTrue("WorkManager carries metadata, not the large media list", next.toString().length() < 1000);
    File retry = PrepareWorker.checkpoint(context, "large-retry");
    assertTrue(retry.length() > 10 * 1024);
    SharedPreferences progress = context.getSharedPreferences("preparations-progress", Context.MODE_PRIVATE);
    progress.edit().putInt("large", 42).putInt("large-failed", 1).putString("large-retry", "3").commit();
    JSONArray resumed = PrepareWorker.readCheckpoint(retry);
    int cursor = PreparationPolicy.checkedCursor(progress.getInt("large", 0), resumed.length());
    assertEquals("fixture:42", resumed.getJSONObject(cursor).getString("url"));
    assertEquals("3", progress.getString("large-retry", ""));
    PrepareWorker.purge(context, "large");
    assertFalse(retry.exists());
  }

  @Test
  public void liveOnlyFailuresHaveAnEmptyRetryInsteadOfResolvingTheWholePlaylistAgain() throws Exception {
    Context context = org.robolectric.RuntimeEnvironment.getApplication();
    JSONObject next = PrepareWorker.completedRequest(context, "live", new JSONObject(),
        new JSONArray().put(new JSONObject().put("live", true)), "", 1);
    assertTrue(next.getBoolean("retry_failed"));
    assertEquals(0, PrepareWorker.readCheckpoint(PrepareWorker.checkpoint(context, "live-retry")).length());
    PrepareWorker.purge(context, "live");
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
