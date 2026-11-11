package dev.ytui.android;

import android.content.Intent;
import android.os.Build;
import android.os.Bundle;
import androidx.activity.ComponentActivity;
import java.io.InputStream;

public final class RustActivity extends ComponentActivity {
  final VideoViewPreparation videoViews = new VideoViewPreparation(this);
  static final java.util.concurrent.atomic.AtomicLong generations =
      new java.util.concurrent.atomic.AtomicLong();
  static final java.util.concurrent.atomic.AtomicLong shutdowns =
      new java.util.concurrent.atomic.AtomicLong();
  private String exportDownload;
  private String exportPlaylist;
  private int documentRequest;
  private long documentAccountIntent;
  private String documentToken;
  /** The launch intent was consumed before this activity was recreated. */
  private boolean restoredIntent;
  static volatile java.lang.ref.WeakReference<RustActivity> reference =
      new java.lang.ref.WeakReference<>(null);

  static RustActivity current() {
    return reference.get();
  }

  @Override
  protected void onCreate(Bundle state) {
    reference = new java.lang.ref.WeakReference<>(this);
    MediaBridge.offlineUi =
        BuildConfig.DEBUG && getIntent().getBooleanExtra("ytui.offline-ui", false);
    super.onCreate(state);
    generations.incrementAndGet();
    ComposeHost.attach(this);
    if (state != null) {
      restoredIntent = true;
      exportDownload = state.getString("ytui-export-download");
      exportPlaylist = state.getString("ytui-export-playlist");
      documentRequest = state.getInt("ytui-document-request");
      documentAccountIntent = state.getLong("ytui-document-account-intent");
      documentToken = state.getString("ytui-document-token");
    }
    OfflineStore.resetSnapshotClient();
    getWindow()
        .getDecorView()
        .setOnApplyWindowInsetsListener(
            (view, insets) -> {
              if (Build.VERSION.SDK_INT >= 30) {
                android.graphics.Insets bars =
                    insets.getInsets(
                        android.view.WindowInsets.Type.systemBars()
                            | android.view.WindowInsets.Type.displayCutout());
                MediaBridge.topInset = bars.top;
                MediaBridge.bottomInset = bars.bottom;
                MediaBridge.leftInset = bars.left;
                MediaBridge.rightInset = bars.right;
              } else {
                MediaBridge.topInset = insets.getSystemWindowInsetTop();
                MediaBridge.bottomInset = insets.getSystemWindowInsetBottom();
                MediaBridge.leftInset = insets.getSystemWindowInsetLeft();
                MediaBridge.rightInset = insets.getSystemWindowInsetRight();
              }
              return insets;
            });
    getWindow().getDecorView().requestApplyInsets();
    // First-run setup asks once it is finished or skipped (Onboarding); otherwise ask now.
    if (!OnboardingStore.showsAtLaunch(this)) NotificationPermission.requestOnce(this);
    onNewIntent(getIntent());
  }

  @Override
  protected void onResume() {
    super.onResume();
    MediaBridge.uiVisible = true;
    PlaybackService.warmForeground();
    videoViews.schedule();
    MediaBridge.notifyUi();
  }

  @Override
  protected void onStop() {
    videoViews.clear();
    if (!FullscreenActivity.visible) {
      MediaBridge.uiVisible = false;
      if (!isChangingConfigurations() && PlaybackService.current != null) {
        PlaybackService.current.pauseForBackground();
        PlaybackService.current.releaseIdle();
      }
    }
    super.onStop();
  }

  @Override
  protected void onDestroy() {
    videoViews.clear();
    if (reference.get() == this) reference.clear();
    super.onDestroy();
    shutdowns.incrementAndGet();
  }

  @Override
  public void onConfigurationChanged(android.content.res.Configuration configuration) {
    // Detached widgets retain resolved font/dimension attributes. Discard an
    // unused view rather than attach stale configuration after rotation/scaling.
    videoViews.clear();
    super.onConfigurationChanged(configuration);
    videoViews.schedule();
  }

  @Override
  protected void onNewIntent(Intent intent) {
    super.onNewIntent(intent);
    setIntent(intent);
    if ("dev.ytui.OPEN_PLAYER".equals(intent.getAction())) MediaBridge.event("show_player", "");
    // A recreated activity (or a relaunch from Recents) gets the share or link it already
    // handled; only a newly delivered intent is played or searched again (AR-08).
    boolean replayed =
        restoredIntent || (intent.getFlags() & Intent.FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY) != 0;
    restoredIntent = false;
    if (replayed) return;
    String shared = null;
    if (Intent.ACTION_SEND.equals(intent.getAction())) {
      CharSequence text = intent.getCharSequenceExtra(Intent.EXTRA_TEXT);
      if (text != null) shared = text.toString();
    } else if (Intent.ACTION_VIEW.equals(intent.getAction()) && intent.getData() != null)
      shared = intent.getData().toString();
    if (shared != null && !shared.trim().isEmpty()) MediaBridge.event("shared", shared.trim());
  }

  @Override
  protected void onSaveInstanceState(Bundle state) {
    state.putString("ytui-export-download", exportDownload);
    state.putString("ytui-export-playlist", exportPlaylist);
    state.putInt("ytui-document-request", documentRequest);
    state.putLong("ytui-document-account-intent", documentAccountIntent);
    state.putString("ytui-document-token", documentToken);
    super.onSaveInstanceState(state);
  }

  void pickDocument(String kind, String token) {
    if (documentRequest != 0) {
      MediaBridge.event("error", "Finish or cancel the open document picker first");
      return;
    }
    documentRequest = "lyrics".equals(kind) ? 44 : "theme".equals(kind) ? 42 : 41;
    documentToken = token;
    documentAccountIntent = documentRequest == 41 ? NativeBridge.beginAccountChange() : 0;
    try {
      startActivityForResult(
          new Intent(Intent.ACTION_OPEN_DOCUMENT)
              .setType("*/*")
              .addCategory(Intent.CATEGORY_OPENABLE),
          documentRequest);
    } catch (RuntimeException error) {
      documentRequest = 0;
      MediaBridge.event("error", "No document provider is available");
    }
  }

  void pickFolder(boolean audio) {
    if (documentRequest != 0 || exportDownload != null || exportPlaylist != null) {
      MediaBridge.event("error", "Finish or cancel the current file picker first");
      return;
    }
    documentRequest = audio ? 46 : 47;
    try {
      startActivityForResult(
          new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE)
              .putExtra(
                  android.provider.DocumentsContract.EXTRA_INITIAL_URI,
                  StorageLocations.initialDocument(audio))
              .addFlags(
                  Intent.FLAG_GRANT_READ_URI_PERMISSION
                      | Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                      | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION
                      | Intent.FLAG_GRANT_PREFIX_URI_PERMISSION),
          documentRequest);
    } catch (RuntimeException error) {
      documentRequest = 0;
      MediaBridge.event("error", "No folder picker is available");
    }
  }

  void exportDownload(String id) {
    if (exportDownload != null || exportPlaylist != null) {
      MediaBridge.event("error", "Finish or cancel the open export picker first");
      return;
    }
    exportDownload = id;
    OfflineStore.IO.execute(
        () -> {
          try {
            OfflineExport.Plan plan = OfflineExport.inspect(id);
            MediaBridge.main.post(
                () -> {
                  if (isFinishing() || isDestroyed()) {
                    exportDownload = null;
                    return;
                  }
                  try {
                    startActivityForResult(
                        new Intent(Intent.ACTION_CREATE_DOCUMENT)
                            .setType(plan.mime)
                            .addCategory(Intent.CATEGORY_OPENABLE)
                            .addFlags(
                                Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                                    | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
                            .putExtra(
                                Intent.EXTRA_TITLE,
                                PlaybackPolicy.exportName(plan.title, plan.extension)),
                        43);
                  } catch (RuntimeException error) {
                    exportDownload = null;
                    MediaBridge.event("error", "No document provider is available for export");
                  }
                });
          } catch (Exception error) {
            MediaBridge.main.post(() -> exportDownload = null);
            MediaBridge.event("error", OfflineExport.safeReason(error));
          }
        });
  }

  void exportPlaylist(String id) {
    if (exportDownload != null || exportPlaylist != null) {
      MediaBridge.event("error", "Finish or cancel the open export picker first");
      return;
    }
    exportPlaylist = id;
    OfflineStore.IO.execute(
        () -> {
          try {
            String title = PlaylistZip.find(id).optString("title");
            MediaBridge.main.post(
                () -> {
                  if (isFinishing() || isDestroyed()) {
                    exportPlaylist = null;
                    return;
                  }
                  try {
                    startActivityForResult(
                        new Intent(Intent.ACTION_CREATE_DOCUMENT)
                            .setType("application/zip")
                            .addCategory(Intent.CATEGORY_OPENABLE)
                            .addFlags(
                                Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                                    | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
                            .putExtra(Intent.EXTRA_TITLE, PlaybackPolicy.archiveName(title)),
                        48);
                  } catch (RuntimeException error) {
                    exportPlaylist = null;
                    MediaBridge.event("error", "No document provider is available for export");
                  }
                });
          } catch (Exception error) {
            MediaBridge.main.post(() -> exportPlaylist = null);
            MediaBridge.event("error", OfflineExport.safeReason(error));
          }
        });
  }

  @Override
  protected void onActivityResult(int request, int result, Intent data) {
    super.onActivityResult(request, result, data);
    if ((request == 46 || request == 47) && request == documentRequest) {
      documentRequest = 0;
      if (result == RESULT_OK && data != null && data.getData() != null)
        OfflineStore.IO.execute(
            () -> {
              try {
                StorageLocations.select(request == 46, data.getData(), data.getFlags());
              } catch (Exception error) {
                MediaBridge.event(
                    "error", "Could not use this folder; select one with read/write access");
              }
            });
      return;
    }
    if (request == 43 || request == 48) {
      String id = request == 43 ? exportDownload : exportPlaylist;
      if (request == 43) exportDownload = null;
      else exportPlaylist = null;
      if (id == null || result != RESULT_OK || data == null || data.getData() == null) return;
      android.net.Uri destination = data.getData();
      if ((data.getFlags() & Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION) != 0) {
        try {
          getContentResolver()
              .takePersistableUriPermission(destination, Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
        } catch (SecurityException ignored) {
        }
      }
      if (request == 43) OfflineExport.enqueue(id, destination);
      else OfflineExport.enqueuePlaylist(id, destination);
      return;
    }
    if (request != documentRequest || documentRequest == 0) return;
    documentRequest = 0;
    final long accountIntent = documentAccountIntent;
    final String token = documentToken;
    documentToken = null;
    if (result != RESULT_OK || data == null || data.getData() == null) return;
    final android.net.Uri uri = data.getData();
    final android.content.ContentResolver resolver = getApplicationContext().getContentResolver();
    OfflineStore.IO.execute(
        () -> {
          try (InputStream input = resolver.openInputStream(uri)) {
            MediaBridge.event(
                request == 41 ? "cookie_file" : request == 42 ? "theme_file" : "lyrics_file",
                DocumentInput.read(input, request == 44 ? 512 * 1024 : 2 * 1024 * 1024),
                token,
                accountIntent);
          } catch (Exception error) {
            MediaBridge.event("error", "Could not read selected document");
          }
        });
  }
}
