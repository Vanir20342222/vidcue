package dev.ytui.android;

import android.app.*;
import android.content.*;
import android.content.pm.ServiceInfo;
import android.net.Uri;
import android.provider.DocumentsContract;
import androidx.media3.exoplayer.offline.Download;
import androidx.media3.exoplayer.offline.DownloadCursor;
import androidx.work.*;
import java.io.*;
import java.util.*;
import java.util.concurrent.TimeUnit;
import java.util.function.BooleanSupplier;
import org.json.*;

/**
 * Converts completed offline audio to MP3 files in the user's Music folder. Reads the cached
 * download only and never contacts a server.
 *
 * <p>The original is never touched until the MP3 is complete and checked. The offline cache copy
 * always stays, so in-app offline playback keeps working. A file already saved to the Music folder
 * is replaced by the MP3 (the library entry then points at the MP3); an item with no saved file
 * gets a new MP3 there.
 */
@androidx.annotation.OptIn(markerClass = androidx.media3.common.util.UnstableApi.class)
public final class Mp3Convert extends Worker {
  private static final String PREFS = "mp3";
  /** One folder decision at a time, so two items of one playlist share a single folder. */
  private static final Object FOLDER_LOCK = new Object();

  enum Outcome {
    CONVERTED,
    ALREADY_DONE
  }

  public Mp3Convert(Context context, WorkerParameters parameters) {
    super(context, parameters);
  }

  private static SharedPreferences prefs() {
    return MediaBridge.context.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
  }

  static boolean converted(String id) {
    return prefs().getBoolean("done:" + id, false);
  }

  /** Whether finished audio downloads are handled by this converter instead of a plain export. */
  static boolean takesOver(String id) {
    try {
      return Mp3Policy.automatic(
          MobileSettings.enabled("auto_mp3"), OfflineStore.cachedRequest(id).optBoolean("audio"));
    } catch (Exception incomplete) {
      return false;
    }
  }

  private static OneTimeWorkRequest request(String mode, String id) {
    Data.Builder input = new Data.Builder().putString("mode", mode);
    if (id != null) input.putString("id", id);
    return new OneTimeWorkRequest.Builder(Mp3Convert.class)
        .setInputData(input.build())
        .addTag("ytui-mp3")
        .build();
  }

  /** A newly finished (or pending) audio download, when automatic conversion is on. */
  static void enqueueAuto(String id) {
    if (!takesOver(id) || converted(id)) return;
    WorkManager.getInstance(MediaBridge.context)
        .enqueueUniqueWork("ytui-mp3-" + id, ExistingWorkPolicy.KEEP, request("item", id));
  }

  /** Every item of one offline playlist. */
  static void enqueuePlaylist(String playlistId) {
    OneTimeWorkRequest work = request("playlist", playlistId);
    OfflineExport.state(MediaBridge.context, work.getId().toString(), "MP3 conversion queued");
    WorkManager.getInstance(MediaBridge.context)
        .enqueueUniqueWork("ytui-mp3-playlist-" + playlistId, ExistingWorkPolicy.KEEP, work);
  }

  /** Everything already downloaded; runs when automatic conversion is switched on. */
  static void enqueueBackfill() {
    OneTimeWorkRequest work = request("backfill", null);
    OfflineExport.state(MediaBridge.context, work.getId().toString(), "MP3 conversion queued");
    WorkManager.getInstance(MediaBridge.context)
        .enqueueUniqueWork("ytui-mp3-backfill", ExistingWorkPolicy.KEEP, work);
  }

  @Override
  public Result doWork() {
    String mode = getInputData().getString("mode"), id = getInputData().getString("id");
    String work = getId().toString();
    boolean quiet = "item".equals(mode);
    try {
      if (mode == null || ("item".equals(mode) || "playlist".equals(mode)) && id == null)
        return Result.failure();
      if (quiet && (!MobileSettings.enabled("auto_mp3") || converted(id))) return Result.success();
      if ("backfill".equals(mode) && !MobileSettings.enabled("auto_mp3")) return Result.success();
      requireFolder();
      setForegroundAsync(foreground()).get(15, TimeUnit.SECONDS);
      List<String[]> jobs = new ArrayList<>(); // id, playlist title or "", position or -1
      if ("playlist".equals(mode)) {
        JSONObject playlist = PlaylistZip.find(id);
        JSONArray items = playlist.optJSONArray("items");
        for (int i = 0; items != null && i < items.length(); i++)
          jobs.add(
              new String[] {
                items.getJSONObject(i).optString("offline_id"),
                playlist.optString("title"),
                String.valueOf(i)
              });
      } else if ("backfill".equals(mode)) {
        for (String candidate : pending()) jobs.add(new String[] {candidate, null, "-1"});
      } else jobs.add(new String[] {id, null, "-1"});
      int done = 0, skipped = 0, failed = 0, already = 0, step = 0;
      for (String[] job : jobs) {
        if (isStopped()) throw new InterruptedIOException();
        if (!quiet)
          OfflineExport.state(
              getApplicationContext(),
              work,
              "Converting to MP3: " + (++step) + " of " + jobs.size());
        try {
          OfflineExport.PlaylistSave save =
              job[1] != null
                  ? new OfflineExport.PlaylistSave(job[1], Integer.parseInt(job[2]))
                  : OfflineExport.playlist(job[0]);
          if (convert(job[0], save, this::isStopped) == Outcome.CONVERTED) done++;
          else already++;
        } catch (UnsupportedOperationException notConvertible) {
          skipped++;
        } catch (InterruptedIOException stopped) {
          throw stopped;
        } catch (Exception error) {
          failed++;
          prefs().edit().putString("error:" + job[0], error.getClass().getSimpleName()).apply();
          android.util.Log.e("YtuiMp3", error.getClass().getName() + " " + error.getMessage());
        }
      }
      if (!quiet) {
        String text = "MP3 conversion complete: " + done + " converted";
        if (already > 0) text += ", " + already + " already MP3";
        if (skipped > 0) text += ", " + skipped + " not convertible";
        if (failed > 0) text += ", " + failed + " failed";
        OfflineExport.state(getApplicationContext(), work, text);
      } else if (failed > 0)
        OfflineExport.state(getApplicationContext(), work, "MP3 conversion failed");
      MediaBridge.event("storage_updated", "");
      if (failed > 0) MediaBridge.event("error", failed + " MP3 conversions failed; try again later");
      return Result.success();
    } catch (Exception error) {
      OfflineExport.state(
          getApplicationContext(),
          work,
          isStopped() ? "MP3 conversion cancelled" : "MP3 conversion failed");
      if (error instanceof UnsupportedOperationException)
        MediaBridge.event("error", error.getMessage());
      else if (!isStopped())
        android.util.Log.e("YtuiMp3", error.getClass().getName() + " " + error.getMessage());
      return Result.failure();
    }
  }

  /** Saved MP3 files need a Music folder the app may still write to. */
  private static void requireFolder() throws IOException {
    Uri tree = StorageLocations.folder(true);
    if (tree == null)
      throw new UnsupportedOperationException("Choose a Music folder in Settings to save MP3 files");
    if (!StorageLocations.granted(tree)) throw new IOException("Music folder permission expired");
  }

  /** Completed audio downloads that have not been converted. */
  static List<String> pending() throws Exception {
    List<Mp3Policy.Candidate> items = new ArrayList<>();
    try (DownloadCursor cursor =
        OfflineStore.manager().getDownloadIndex().getDownloads(Download.STATE_COMPLETED)) {
      while (cursor.moveToNext()) {
        String id = cursor.getDownload().request.id;
        if (id.endsWith("-audio")) continue;
        try {
          boolean audio = OfflineStore.cachedRequest(id).optBoolean("audio");
          JSONObject record = StorageLocations.record(id);
          items.add(
              new Mp3Policy.Candidate(
                  id, audio, record == null ? null : record.optString("uri"), converted(id)));
        } catch (Exception incomplete) {
          // A download without its request is not convertible yet.
        }
      }
    }
    return Mp3Policy.backfill(items);
  }

  /** Converts one item; see the class comment for what happens to the original. */
  static Outcome convert(String id, OfflineExport.PlaylistSave playlist, BooleanSupplier cancelled)
      throws Exception {
    JSONObject record = StorageLocations.record(id);
    if (record != null && Mp3Policy.isMp3(record.optString("uri"))) {
      markDone(id);
      return Outcome.ALREADY_DONE;
    }
    requireFolder();
    OfflineExport.Plan plan = OfflineExport.inspect(id);
    if (plan.parts.size() != 1 || !plan.mime.startsWith("audio/"))
      throw new UnsupportedOperationException("Only completed audio downloads can be converted");
    JSONObject request = OfflineStore.cachedRequest(id);
    Context context = MediaBridge.context;
    long needed = Math.addExact(16L * 1024 * 1024, Math.multiplyExact(plan.parts.get(0).contentLength, 8L));
    if (context.getCacheDir().getUsableSpace() < needed)
      throw new IOException("Not enough temporary space");
    File staged = File.createTempFile("ytui-mp3-", ".mp3", context.getCacheDir());
    try {
      long[] result = encode(plan, staged, cancelled);
      verify(staged, result[0], (int) result[1], (int) result[2]);
      synchronized (FOLDER_LOCK) {
        publish(id, request, record, playlist, plan.title, staged, cancelled);
      }
      markDone(id);
      return Outcome.CONVERTED;
    } finally {
      staged.delete();
    }
  }

  private static void markDone(String id) {
    prefs().edit().putBoolean("done:" + id, true).remove("error:" + id).apply();
  }

  /** Decodes the cached download into [file]. Returns {frames, input rate, bitrate kbit/s}. */
  private static long[] encode(OfflineExport.Plan plan, File file, BooleanSupplier cancelled)
      throws Exception {
    long[] stats = new long[3];
    try (OfflineExport.CachedInput input = new OfflineExport.CachedInput(plan.parts.get(0));
        OutputStream output = new BufferedOutputStream(new FileOutputStream(file), 1 << 16)) {
      Mp3Encoder[] encoder = new Mp3Encoder[1];
      try {
        stats[0] =
            Mp3Decoder.decode(
                input,
                new Mp3Decoder.Sink() {
                  @Override
                  public void format(int sampleRate, int channels) throws IOException {
                    if (encoder[0] != null && stats[1] == sampleRate) return;
                    if (encoder[0] != null)
                      throw new IOException("Audio format changed during conversion");
                    encoder[0] = new Mp3Encoder(sampleRate, channels, Mp3Encoder.BITRATE_KBPS, output);
                    stats[1] = sampleRate;
                    stats[2] = encoder[0].bitrateKbps();
                  }

                  @Override
                  public void pcm(short[] data, int frames) throws IOException {
                    encoder[0].write(data, 0, frames);
                  }
                },
                cancelled);
        if (encoder[0] == null || stats[0] <= 0) throw new IOException("No audio was decoded");
        encoder[0].finish();
      } finally {
        if (encoder[0] != null) encoder[0].close();
      }
    }
    return stats;
  }

  /** Re-reads the staged file: it must start with an MP3 frame and be a believable size. */
  private static void verify(File file, long frames, int sampleRate, int kbps) throws IOException {
    byte[] head = new byte[4096];
    int count;
    try (InputStream in = new FileInputStream(file)) {
      count = in.read(head);
    }
    if (Mp3Policy.firstFrame(head, Math.max(count, 0)) == null)
      throw new IOException("The converted file is not valid MP3");
    if (!Mp3Policy.plausibleSize(file.length(), frames, sampleRate, kbps))
      throw new IOException("The converted file has an unexpected length");
  }

  /**
   * Copies the checked MP3 into the Music folder, repoints the library entry, then removes the
   * previously saved original. A failure before the repoint leaves the original untouched.
   */
  private static void publish(
      String id,
      JSONObject request,
      JSONObject record,
      OfflineExport.PlaylistSave playlist,
      String title,
      File staged,
      BooleanSupplier cancelled)
      throws Exception {
    ContentResolver resolver = MediaBridge.context.getContentResolver();
    Uri tree = StorageLocations.folder(true);
    Uri parent = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree));
    Uri playlistFolder = null;
    String saved = record == null ? "" : record.optString("playlistFolder", "");
    if (!saved.isEmpty()) playlistFolder = Uri.parse(saved);
    else if (playlist != null) playlistFolder = siblingFolder(playlist.title);
    if (playlist != null && playlistFolder == null) {
      playlistFolder =
          DocumentsContract.createDocument(
              resolver,
              parent,
              DocumentsContract.Document.MIME_TYPE_DIR,
              PlaybackPolicy.folderName(playlist.title));
      if (playlistFolder == null) throw new IOException("Could not create the playlist folder");
    }
    if (playlistFolder != null) parent = playlistFolder;
    Uri destination =
        DocumentsContract.createDocument(
            resolver, parent, "audio/mpeg", Mp3Policy.fileName(title, playlist == null ? -1 : playlist.position));
    if (destination == null) throw new IOException("Could not create the MP3 file");
    boolean repointed = false;
    try {
      try (InputStream in = new FileInputStream(staged);
          OutputStream out = resolver.openOutputStream(destination, "w")) {
        if (out == null) throw new IOException("No output stream");
        byte[] buffer = new byte[65536];
        int count;
        while ((count = in.read(buffer)) != -1) {
          if (cancelled.getAsBoolean()) throw new InterruptedIOException();
          out.write(buffer, 0, count);
        }
      }
      try (android.content.res.AssetFileDescriptor descriptor = resolver.openAssetFileDescriptor(destination, "r")) {
        if (descriptor == null || descriptor.getLength() != staged.length())
          throw new IOException("The saved MP3 is incomplete");
      }
      StorageLocations.remember(
          id,
          destination,
          playlistFolder,
          playlist == null ? "" : playlist.title,
          request);
      repointed = true;
    } finally {
      if (!repointed)
        try {
          DocumentsContract.deleteDocument(resolver, destination);
        } catch (Exception ignored) {
        }
    }
    if (record != null) {
      try {
        // The MP3 now stands in for the saved original; a leftover original is only wasted space.
        DocumentsContract.deleteDocument(resolver, Uri.parse(record.getString("uri")));
      } catch (Exception leftover) {
        android.util.Log.w("YtuiMp3", "Original file could not be removed");
      }
    }
  }

  /** The folder an earlier save of the playlist named [title] already created, if any. */
  private static Uri siblingFolder(String title) throws Exception {
    JSONObject files = StorageLocations.records();
    String label = StorageLocations.label(true) + " › " + title;
    for (Iterator<String> keys = files.keys(); keys.hasNext(); ) {
      JSONObject file = files.getJSONObject(keys.next());
      String folder = file.optString("playlistFolder", "");
      if (!folder.isEmpty() && label.equals(file.optString("folder"))) return Uri.parse(folder);
    }
    return null;
  }

  private ForegroundInfo foreground() {
    NotificationManager manager =
        getApplicationContext().getSystemService(NotificationManager.class);
    manager.createNotificationChannel(
        new NotificationChannel(
            "ytui-exports", "Media exports", NotificationManager.IMPORTANCE_LOW));
    Notification notification =
        new Notification.Builder(getApplicationContext(), "ytui-exports")
            .setSmallIcon(R.drawable.ic_vidcue_notification)
            .setContentTitle("Vidcue: converting audio to MP3")
            .setContentText("Converting completed downloads in the background")
            .setOngoing(true)
            .build();
    return android.os.Build.VERSION.SDK_INT >= 29
        ? new ForegroundInfo(
            NotificationIds.export(getId()),
            notification,
            ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        : new ForegroundInfo(NotificationIds.export(getId()), notification);
  }
}
