package dev.ytui.android;

import android.app.*;
import android.content.*;
import android.content.pm.ServiceInfo;
import androidx.work.*;
import java.util.concurrent.*;
import org.json.*;

/** Resolves every playlist item off the UI thread; checkpointed between worker runs. */
@androidx.annotation.OptIn(markerClass = androidx.media3.common.util.UnstableApi.class)
public final class PrepareWorker extends Worker {
  private volatile long activeRequest;

  public PrepareWorker(Context context, WorkerParameters parameters) {
    super(context, parameters);
  }

  static void enqueue(JSONObject request) {
    enqueue(request, null);
  }

  /**
   * Queues preparation of [request] under the job key [key] (a new key when null). The key names
   * the downloads row, checkpoint and progress cursor, so re-queuing an existing key replaces its
   * work and resumes where it stopped instead of adding a second row that starts over.
   */
  static void enqueue(JSONObject request, String key) {
    OneTimeWorkRequest.Builder builder =
        new OneTimeWorkRequest.Builder(PrepareWorker.class)
            .setConstraints(
                new Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
            .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 60, TimeUnit.SECONDS)
            .addTag("ytui-prepare");
    if (key == null) key = java.util.UUID.randomUUID().toString();
    OneTimeWorkRequest work =
        builder
            .setInputData(
                new Data.Builder()
                    .putString("request", request.toString())
                    .putString("key", key)
                    .build())
            .build();
    status(MediaBridge.context, key, "Preparing playlist", request.toString());
    WorkManager manager = WorkManager.getInstance(MediaBridge.context);
    // Rows from before job keys ran as plain work whose id is the key.
    cancelLegacy(manager, key);
    manager.enqueueUniqueWork(
        PreparationPolicy.uniqueWork(key), ExistingWorkPolicy.REPLACE, work);
  }

  /** Stops the work behind job [key] without touching its row or checkpoint. */
  static void cancel(Context context, String key) {
    WorkManager manager = WorkManager.getInstance(context);
    manager.cancelUniqueWork(PreparationPolicy.uniqueWork(key));
    cancelLegacy(manager, key);
  }

  private static void cancelLegacy(WorkManager manager, String key) {
    try {
      manager.cancelWorkById(java.util.UUID.fromString(key));
    } catch (IllegalArgumentException notAWorkId) {
    }
  }

  /** Deletes job [key]'s checkpoint file and progress cursor (its row is separate). */
  static void purge(Context context, String key) {
    SharedPreferences.Editor progress =
        context.getSharedPreferences("preparations-progress", Context.MODE_PRIVATE).edit();
    for (String name : PreparationPolicy.progressKeys(key)) progress.remove(name);
    progress.apply();
    java.io.File checkpoint = checkpoint(context, key);
    if (checkpoint.exists()) checkpoint.delete();
  }

  static java.io.File checkpoint(Context context, String key) {
    return new java.io.File(context.getFilesDir(), "prepare-" + key + ".json");
  }

  /** Status updates from a cancelled or replaced run are dropped. */
  private void report(String id, String text, String request) {
    if (!isStopped()) status(getApplicationContext(), id, text, request);
  }

  /** A stopped run exits; when its row was removed meanwhile, clean what it may have written. */
  private Result stopped(String id) {
    if (!getApplicationContext()
        .getSharedPreferences("preparations", Context.MODE_PRIVATE)
        .contains(id + "-request")) purge(getApplicationContext(), id);
    return Result.retry();
  }

  /** Retries with backoff, or after the last allowed run stops with a final status. */
  private Result retryOrStop(String id, String retrying, String raw) {
    if (PreparationPolicy.retryAllowed(getRunAttemptCount())) {
      report(id, retrying, raw);
      return Result.retry();
    }
    report(
        id,
        "Playlist preparation stopped after "
            + PreparationPolicy.MAX_RUNS
            + " attempts; retry from downloads",
        raw);
    return Result.failure();
  }

  static void status(Context context, String id, String text, String request) {
    context
        .getSharedPreferences("preparations", Context.MODE_PRIVATE)
        .edit()
        .putString(id, text)
        .putString(id + "-request", request)
        .apply();
  }

  @Override
  public Result doWork() {
    String raw = getInputData().getString("request"), id = getInputData().getString("key");
    if (id == null) id = getId().toString();
    if (raw == null) return Result.failure();
    java.io.File checkpoint = checkpoint(getApplicationContext(), id);
    try {
      setForegroundAsync(foreground()).get(15, TimeUnit.SECONDS);
      JSONObject request = new JSONObject(raw);
      JSONArray items;
      if (checkpoint.isFile()) {
        try (java.io.InputStream in = new android.util.AtomicFile(checkpoint).openRead()) {
          java.io.ByteArrayOutputStream out = new java.io.ByteArrayOutputStream();
          byte[] b = new byte[8192];
          int n;
          while ((n = in.read(b)) != -1) {
            out.write(b, 0, n);
            if (out.size() > 16 * 1024 * 1024)
              throw new java.io.IOException("Playlist exceeds budget");
          }
          items = new JSONArray(out.toString("UTF-8"));
        }
      } else {
        JSONObject response =
            call(
                new JSONObject()
                    .put("op", "collection")
                    .put("request", request.get("request"))
                    .put("music", request.optBoolean("music")));
        items = response.getJSONArray("data");
        android.util.AtomicFile atomic = new android.util.AtomicFile(checkpoint);
        java.io.FileOutputStream out = null;
        try {
          out = atomic.startWrite();
          out.write(items.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8));
          atomic.finishWrite(out);
        } catch (Exception error) {
          if (out != null) atomic.failWrite(out);
          throw error;
        }
      }
      SharedPreferences progress =
          getApplicationContext()
              .getSharedPreferences("preparations-progress", Context.MODE_PRIVATE);
      if (request.optBoolean("create_offline_playlist")) {
        JSONObject saved = call(offlinePlaylist(request, items));
        if (!saved.optBoolean("ok")) {
          String error = saved.optString("error", "Cannot create offline playlist");
          report(id, error, raw);
          MediaBridge.event("error", error);
          purge(getApplicationContext(), id);
          return Result.failure();
        }
        MediaBridge.event("offline_playlists_changed", "");
      }
      int start = PreparationPolicy.checkedCursor(progress.getInt(id, 0), items.length()),
          failed = progress.getInt(id + "-failed", 0);
      for (int i = start; i < items.length(); i++) {
        if (isStopped()) return stopped(id);
        JSONObject media = items.getJSONObject(i);
        if (PreparationPolicy.collection(media.optString("kind"))) {
          // Older checkpoints may list nested playlists; they are not single videos.
          if (!progress.edit().putInt(id, i + 1).commit())
            throw new java.io.IOException("Cannot persist playlist progress");
          continue;
        }
        report(
            id, "Preparing " + (i + 1) + " / " + items.length() + " · " + failed + " failed", raw);
        JSONObject resolved =
            call(
                new JSONObject()
                    .put("op", "resolve")
                    .put("media", media)
                    .put("audio", request.optBoolean("audio"))
                    .put("quality", request.optString("quality", "1080")));
        if (isStopped()) return stopped(id);
        if (resolved.optBoolean("ok")
            && (media.optBoolean("live") || resolved.getJSONObject("data").optBoolean("live"))) {
          failed++;
          MediaBridge.event("error", "Skipped a live entry: recording is not supported");
        } else if (!resolved.optBoolean("ok")) {
          String error = resolved.optString("error").toLowerCase(java.util.Locale.ROOT);
          if (PreparationPolicy.rateLimited(error))
            return retryOrStop(id, "Service cooldown; playlist preparation will retry", raw);
          failed++;
          MediaBridge.event(
              "error",
              "Playlist entry failed: "
                  + media.optString("title")
                  + " · "
                  + resolved.optString("error"));
        } else {
          JSONObject ready =
              new JSONObject()
                  .put("media", media)
                  .put("stream", resolved.getJSONObject("data"))
                  .put("quality", request.optString("quality", "1080"))
                  .put("audio", request.optBoolean("audio"));
          CompletableFuture<Boolean> accepted = new CompletableFuture<>();
          MediaBridge.main.post(
              () -> {
                try {
                  accepted.complete(OfflineStore.add(ready));
                } catch (Exception e) {
                  accepted.completeExceptionally(e);
                }
              });
          if (!accepted.get(15, TimeUnit.SECONDS)) {
            // Low storage: keep the checkpoint so a retry resumes at this entry.
            report(id, "Not enough free space; free some space and retry from downloads", raw);
            return Result.failure();
          }
          String downloadId = OfflineStore.id(ready);
          long deadline = android.os.SystemClock.elapsedRealtime() + 15000;
          while (!OfflineStore.persisted(downloadId, ready)) {
            if (isStopped() || android.os.SystemClock.elapsedRealtime() > deadline)
              throw new java.io.IOException(
                  "Download index did not acknowledge the playlist entry");
            Thread.sleep(100);
          }
        }
        if (!progress.edit().putInt(id, i + 1).putInt(id + "-failed", failed).commit())
          throw new java.io.IOException("Cannot persist playlist progress");
      }
      report(
          id,
          failed == 0
              ? "Playlist queued: " + items.length() + " entries"
              : "Playlist queued with " + failed + " failures; review before retrying",
          raw);
      purge(getApplicationContext(), id);
      return failed == 0 ? Result.success() : Result.failure();
    } catch (IllegalArgumentException error) {
      report(id, "Invalid playlist checkpoint; remove and add the playlist again", raw);
      purge(getApplicationContext(), id);
      return Result.failure();
    } catch (Exception error) {
      if (isStopped()) return stopped(id);
      return retryOrStop(id, "Playlist preparation interrupted; it will retry", raw);
    }
  }

  private JSONObject call(JSONObject request) throws Exception {
    activeRequest = System.nanoTime() & Long.MAX_VALUE;
    request.put("request_id", activeRequest);
    return new JSONObject(NativeBridge.request(request.toString()));
  }

  /** The same collection updates one playlist on retries and repeat downloads. */
  static JSONObject offlinePlaylist(JSONObject request, JSONArray items) throws Exception {
    String source = request.getJSONObject("request").getString("Collection");
    String playlistId = java.util.UUID.nameUUIDFromBytes(
        ("vidcue:download-playlist:" + source)
            .getBytes(java.nio.charset.StandardCharsets.UTF_8)).toString();
    JSONArray members = new JSONArray();
    java.util.HashSet<String> seen = new java.util.HashSet<>();
    for (int i = 0; i < items.length(); i++) {
      JSONObject media = new JSONObject(items.getJSONObject(i).toString());
      if (media.optBoolean("live") || PreparationPolicy.collection(media.optString("kind"))
          || !seen.add(media.getString("url"))) continue;
      JSONObject download = new JSONObject().put("media", media)
          .put("audio", request.optBoolean("audio"))
          .put("quality", request.optString("quality", "1080"));
      media.put("offline_id", OfflineStore.id(download));
      members.put(media);
    }
    String title = request.optString("title", "Downloaded playlist").trim();
    title = title.codePoints().filter(c -> !Character.isISOControl(c)).limit(120)
        .collect(StringBuilder::new, StringBuilder::appendCodePoint, StringBuilder::append)
        .toString().trim();
    if (title.isEmpty()) title = "Downloaded playlist";
    return new JSONObject().put("op", "offline_playlist_download").put("id", playlistId)
        .put("title", title).put("items", members);
  }

  @Override
  public void onStopped() {
    super.onStopped();
    if (activeRequest > 0) NativeBridge.cancel(activeRequest);
  }

  @Override
  public com.google.common.util.concurrent.ListenableFuture<ForegroundInfo>
      getForegroundInfoAsync() {
    return com.google.common.util.concurrent.Futures.immediateFuture(foreground());
  }

  private ForegroundInfo foreground() {
    NotificationManager manager =
        getApplicationContext().getSystemService(NotificationManager.class);
    manager.createNotificationChannel(
        new NotificationChannel(
            "ytui-prepare", "Playlist preparation", NotificationManager.IMPORTANCE_LOW));
    Notification n =
        new Notification.Builder(getApplicationContext(), "ytui-prepare")
            .setSmallIcon(dev.ytui.android.R.drawable.ic_vidcue_notification)
            .setContentTitle("Vidcue: preparing playlist")
            .setContentText("Preparing every entry for offline download")
            .setOngoing(true)
            .build();
    return android.os.Build.VERSION.SDK_INT >= 29
        ? new ForegroundInfo(
            NotificationIds.preparation(getId()), n, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        : new ForegroundInfo(NotificationIds.preparation(getId()), n);
  }
}
