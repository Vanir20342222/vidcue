package dev.ytui.android;

import android.content.*;
import android.content.res.Configuration;
import android.graphics.SurfaceTexture;
import android.os.*;
import java.util.concurrent.ConcurrentLinkedQueue;
import org.json.*;

/** Android platform adapters; providers, theme palettes and durable application state are Rust. */
@androidx.annotation.OptIn(markerClass = androidx.media3.common.util.UnstableApi.class)
public final class MediaBridge {
  static void recordResolutionTimings(long token, org.json.JSONObject stream) {
    StartupTimings.cacheState(token, stream.optString("resolution_cache_state"));
    for (String phase : StartupTimings.PHASES)
      StartupTimings.stage(token, phase, stream.optLong("resolution_" + phase + "_ms", -1));
  }

  static android.app.Application context;
  static final Handler main = new Handler(Looper.getMainLooper());
  static final ConcurrentLinkedQueue<JSONObject> events = new ConcurrentLinkedQueue<>();
  // Keep accepted starts ordered across Activity and service recreation.
  static final java.util.concurrent.ExecutorService playbackStore =
      java.util.concurrent.Executors.newSingleThreadExecutor();
  static volatile String playerSnapshot = "{}";
  // Always null since the egui GLES video path was retired; PlaybackService still reads it.
  static SurfaceTexture texture;
  static volatile int videoWidth = 1280, videoHeight = 720;
  static volatile int topInset, bottomInset, leftInset, rightInset;
  static volatile boolean uiVisible;
  static volatile boolean offlineUi;

  public static String uiMode() {
    return offlineUi ? "offline" : "network";
  }

  static void notifyUi() {
    if (uiVisible) ComposeHost.changed();
  }

  static final java.util.concurrent.atomic.AtomicLong videoFrames =
      new java.util.concurrent.atomic.AtomicLong();

  static void attach(Context ctx) {
    context = (android.app.Application) ctx.getApplicationContext();
    main.postDelayed(ScriptRuntime::warm, 800);
    main.post(
        () -> {
          try {
            OfflineStore.manager();
          } catch (Exception error) {
            event("error", "Offline library could not be restored");
          }
        });
    try {
      NetworkSettings.apply(
          new JSONObject(NativeBridge.request("{\"op\":\"settings\"}")).getJSONObject("data"));
    } catch (Exception ignored) {
    }
    PlaybackHttp.warm();
  }

  static void event(String kind, String text) {
    event(kind, text, null, 0);
  }

  static void event(String kind, String text, String token, long accountIntent) {
    // Success/progress feedback belongs in controls and library state, never an overlay.
    if ("notice".equals(kind)) return;
    try {
      events.add(
          new JSONObject()
              .put("kind", kind)
              .put("text", text == null ? "" : text)
              .put("request_token", token)
              .put("account_intent", accountIntent));
      ComposeHost.changed();
    } catch (JSONException ignored) {
    }
  }

  public static String evaluate(String script) {
    return ScriptRuntime.evaluate(script);
  }

  public static String sealAccount(String text) throws Exception {
    return AccountCipher.seal(text);
  }

  public static String openAccount(String envelope) throws Exception {
    return AccountCipher.open(envelope);
  }

  public static String snapshot() {
    try {
      JSONObject value =
          new JSONObject()
              .put("app_version", BuildConfig.VERSION_NAME + " (" + BuildConfig.VERSION_CODE + ")")
              .put("inset_top", topInset)
              .put("inset_bottom", bottomInset)
              .put("inset_left", leftInset)
              .put("inset_right", rightInset);
      value.put(
          "dark",
          (context.getResources().getConfiguration().uiMode & Configuration.UI_MODE_NIGHT_MASK)
              == Configuration.UI_MODE_NIGHT_YES);
      JSONObject player = new JSONObject(playerSnapshot);
      if (player.optJSONObject("media") == null && !offlineUi) {
        JSONObject recovered = PlaybackRecovery.snapshot();
        if (recovered != null) player = recovered;
      }
      if (AudioTap.enabled) {
        long packed = AudioTap.levels;
        player
            .put("left", (packed & 65535) / 65535.0)
            .put("right", ((packed >>> 16) & 65535) / 65535.0)
            .put("peak", ((packed >>> 32) & 65535) / 65535.0);
      }
      value.put("player", player);
      JSONArray downloads = OfflineStore.changedSnapshot();
      if (downloads != null) value.put("downloads", downloads);
      JSONArray pending = new JSONArray();
      for (int i = 0; i < 32; i++) {
        JSONObject event = events.poll();
        if (event == null) break;
        pending.put(event);
      }
      value.put("events", pending);
      return value.toString();
    } catch (Exception error) {
      return "{}";
    }
  }

  public static void execute(String raw) {
    main.post(
        () -> {
          try {
            JSONObject request = new JSONObject(raw);
            String op = request.optString("op");
            switch (op) {
              case "theme":
                {
                  RustActivity activity = RustActivity.current();
                  if (activity != null) {
                    boolean light = request.optBoolean("light");
                    org.json.JSONArray rgb = request.getJSONArray("background");
                    int color =
                        android.graphics.Color.rgb(rgb.getInt(0), rgb.getInt(1), rgb.getInt(2));
                    activity.getWindow().setStatusBarColor(color);
                    activity.getWindow().setNavigationBarColor(color);
                    if (android.os.Build.VERSION.SDK_INT >= 30) {
                      android.view.WindowInsetsController controller =
                          activity.getWindow().getInsetsController();
                      if (controller != null) {
                        int flags =
                            android.view.WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS
                                | android.view.WindowInsetsController
                                    .APPEARANCE_LIGHT_NAVIGATION_BARS;
                        controller.setSystemBarsAppearance(light ? flags : 0, flags);
                      }
                    } else {
                      android.view.View decor = activity.getWindow().getDecorView();
                      int flags =
                          android.view.View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR
                              | android.view.View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR;
                      decor.setSystemUiVisibility(
                          light
                              ? decor.getSystemUiVisibility() | flags
                              : decor.getSystemUiVisibility() & ~flags);
                    }
                  }
                  break;
                }
              case "background":
                if (RustActivity.current() != null) RustActivity.current().moveTaskToBack(true);
                break;
              case "play":
                {
                  JSONObject playRequest = request.getJSONObject("request");
                  // A delayed resolution must not create a foreground-service promise
                  // for a request which policy will immediately leave paused.
                  if (!PlaybackPolicy.mayAutoPlay(
                      uiVisible,
                      MobileSettings.enabled("background_playback"),
                      playRequest.optBoolean("preview"))) {
                    long timing = playRequest.optLong("startup_token");
                    StartupTimings.ended(timing, "cancelled", SystemClock.elapsedRealtime());
                    event(
                        "playback_start_failed",
                        "Return to the app and tap Play to continue.",
                        Long.toString(timing),
                        0);
                    break;
                  }
                  Intent intent =
                      new Intent(context, PlaybackService.class)
                          .setAction("play")
                          .putExtra("request", playRequest.toString());
                  try {
                    // Reject malformed descriptors before creating an Android
                    // foreground-service obligation or interrupting current media.
                    PlaybackService.validateRequest(playRequest);
                    context.startForegroundService(intent);
                  } catch (JSONException invalid) {
                    long timing = playRequest.optLong("startup_token");
                    StartupTimings.ended(timing, "failed", SystemClock.elapsedRealtime());
                    event(
                        "playback_start_failed",
                        "Invalid playback request",
                        Long.toString(timing),
                        0);
                  } catch (RuntimeException unavailable) {
                    long timing = playRequest.optLong("startup_token");
                    StartupTimings.ended(timing, "failed", SystemClock.elapsedRealtime());
                    event(
                        "playback_start_failed",
                        "Playback could not start. Try again.",
                        Long.toString(timing),
                        0);
                  }
                  break;
                }
              case "download_playlist":
                PrepareWorker.enqueue(request);
                break;
              case "download":
                OfflineStore.add(request.getJSONObject("request"));
                break;
              case "offline":
                OfflineStore.IO.execute(
                    () -> {
                      try {
                        JSONObject item = OfflineStore.offlineRequest(request.getString("id"));
                        if (request.has("context"))
                          item.put("context", request.getJSONObject("context"));
                        if (request.has("context_id"))
                          item.put("context_id", request.getString("context_id"));
                        // Use the same guarded handoff as online playback: the file
                        // lookup may finish after the activity has gone to the background.
                        execute(new JSONObject().put("op", "play").put("request", item).toString());
                      } catch (Exception error) {
                        event(
                            "error",
                            "Offline media is unavailable; reconnect its folder or save it again");
                      }
                    });
                break;
              case "playlist_folder":
                // The lookup reads the offline index, so it runs off the UI thread like "offline".
                OfflineStore.IO.execute(
                    () -> {
                      try {
                        android.net.Uri folder =
                            StorageLocations.playlistFolder(request.getString("id"));
                        if (folder == null) {
                          event("error", "Save a playlist download to find its folder here");
                          return;
                        }
                        context.startActivity(
                            new Intent(Intent.ACTION_VIEW)
                                .setDataAndType(
                                    folder,
                                    android.provider.DocumentsContract.Document.MIME_TYPE_DIR)
                                .addFlags(
                                    Intent.FLAG_GRANT_READ_URI_PERMISSION
                                        | Intent.FLAG_ACTIVITY_NEW_TASK));
                      } catch (ActivityNotFoundException noFileManager) {
                        event("error", "No file manager can open this folder");
                      } catch (Exception unavailable) {
                        event("error", "Playlist folder is unavailable; reconnect its folder");
                      }
                    });
                break;
              case "folder_pick":
                if (RustActivity.current() != null)
                  RustActivity.current().pickFolder(request.optBoolean("audio"));
                break;
              case "folder_retry":
                OfflineStore.exportPending();
                break;
              case "library_visible":
                OfflineStore.libraryVisible = request.optBoolean("value");
                if (OfflineStore.libraryVisible) OfflineStore.dirty = true;
                break;
              case "download_remove":
                OfflineStore.remove(request.getString("id"));
                break;
              case "download_toggle":
                OfflineStore.toggle(request.getString("id"));
                break;
              case "download_export":
                if (RustActivity.current() != null)
                  RustActivity.current().exportDownload(request.getString("id"));
                break;
              case "playlist_export":
                if (RustActivity.current() != null)
                  RustActivity.current().exportPlaylist(request.getString("id"));
                break;
              case "history_clear":
                PlaybackRecovery.clear();
                notifyUi();
                playbackStore.execute(
                    () -> {
                      try {
                        PlaybackService.rpcArray(new JSONObject().put("op", "history_clear"));
                        event("history_cleared", "");
                      } catch (Exception error) {
                        event("error", "Could not clear playback history");
                      }
                    });
                break;
              case "open":
                context.startActivity(
                    new Intent(Intent.ACTION_VIEW, android.net.Uri.parse(request.getString("url")))
                        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
                break;
              case "pick":
                {
                  RustActivity activity = RustActivity.current();
                  if (activity != null)
                    activity.pickDocument(
                        request.optString("kind"), request.optString("request_token"));
                  break;
                }
              case "login":
                if (NetworkSettings.explicitProxy) {
                  event("error", "Use cookie-file import when an explicit proxy is configured");
                  break;
                }
                context.startActivity(
                    new Intent(context, AuthActivity.class)
                        .putExtra("ytui.offline-auth", BuildConfig.DEBUG && offlineUi)
                        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
                break;
              case "disconnect":
                PlaybackRecovery.clear();
                android.webkit.CookieManager.getInstance().removeAllCookies(null);
                if (PlaybackService.current != null) PlaybackService.current.stopAndClear();
                break;
              case "settings":
                if (!request.getJSONObject("config").optBoolean("playback_history", true))
                  PlaybackRecovery.clear();
                NetworkSettings.apply(request.getJSONObject("config"));
                if (PlaybackService.current != null) PlaybackService.current.command(request);
                break;
              case "stop":
                PlaybackRecovery.clear();
                if (PlaybackService.current != null) PlaybackService.current.stopAndClear();
                notifyUi();
                break;
              default:
                if (PlaybackService.current != null) PlaybackService.current.command(request);
                break;
            }
          } catch (Exception error) {
            event("error", "Android operation failed: " + error.getClass().getSimpleName());
          }
        });
  }

  private MediaBridge() {}
}
