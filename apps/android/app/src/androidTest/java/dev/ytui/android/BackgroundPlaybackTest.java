package dev.ytui.android;

import static org.junit.Assert.*;

import android.content.Context;
import androidx.lifecycle.Lifecycle;
import androidx.test.core.app.ActivityScenario;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import androidx.test.platform.app.InstrumentationRegistry;
import java.io.File;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import org.json.JSONObject;
import org.junit.*;
import org.junit.runner.RunWith;

/** Real service/activity transitions on a disposable install, using only generated local audio. */
@RunWith(AndroidJUnit4.class)
@androidx.annotation.OptIn(markerClass = androidx.media3.common.util.UnstableApi.class)
public final class BackgroundPlaybackTest {
  private JSONObject mobile;
  private JSONObject request;

  @Rule
  public androidx.test.rule.GrantPermissionRule notifications =
      androidx.test.rule.GrantPermissionRule.grant(
          android.os.Build.VERSION.SDK_INT >= 33
              ? new String[] {"android.permission.POST_NOTIFICATIONS"}
              : new String[] {});

  @Before
  public void setup() throws Exception {
    mobile = MobileSettings.snapshot();
    main(
        () -> {
          if (PlaybackService.current != null) PlaybackService.current.stopAndClear();
        });
    MediaRuntimeTest.await("Previous service closes", false, () -> PlaybackService.current == null);
    Context context = InstrumentationRegistry.getInstrumentation().getTargetContext();
    File file = new File(context.getCacheDir(), "background-fixture.m4a");
    try (InputStream input =
        InstrumentationRegistry.getInstrumentation()
            .getContext()
            .getAssets()
            .open("fixture-audio.m4a")) {
      Files.copy(input, file.toPath(), StandardCopyOption.REPLACE_EXISTING);
    }
    request =
        new JSONObject()
            .put(
                "media",
                new JSONObject()
                    .put("url", "https://media.invalid/background-fixture")
                    .put("title", "Background fixture")
                    .put("kind", "song"))
            .put("stream", new JSONObject().put("url", android.net.Uri.fromFile(file).toString()))
            .put("audio", true);
  }

  @After
  public void cleanup() throws Exception {
    main(
        () -> {
          if (PlaybackService.current != null) PlaybackService.current.stopAndClear();
        });
    MobileSettings.save(mobile);
    PlaybackRecovery.clear();
  }

  private void main(Runnable action) {
    InstrumentationRegistry.getInstrumentation().runOnMainSync(action);
  }

  private PlaybackService start(boolean background) throws Exception {
    MobileSettings.save(new JSONObject().put("background_playback", background));
    MediaBridge.execute(new JSONObject().put("op", "play").put("request", request).toString());
    MediaRuntimeTest.await("Fixture starts", () -> MediaRuntimeTest.status().optBoolean("playing"));
    PlaybackService service = PlaybackService.current;
    main(() -> service.player.setRepeatMode(androidx.media3.common.Player.REPEAT_MODE_ONE));
    return service;
  }

  @Test
  public void enabledPlaybackKeepsTheSameServiceAndAdvancesWhileActivityIsStopped()
      throws Exception {
    try (ActivityScenario<RustActivity> scene = RuntimeTest.launchOffline()) {
      PlaybackService service = start(true);
      long generation = service.generation;
      scene.moveToState(Lifecycle.State.CREATED);
      double before = MediaRuntimeTest.status().optDouble("position");
      MediaRuntimeTest.await(
          "Background audio advances",
          () ->
              MediaRuntimeTest.status().optBoolean("playing")
                  && MediaRuntimeTest.status().optDouble("position") > before + 0.3);
      assertSame(service, PlaybackService.current);
      scene.moveToState(Lifecycle.State.RESUMED);
      assertSame(service, PlaybackService.current);
      assertEquals(generation, service.generation);
      assertNotNull(PlaybackRecovery.snapshot());
    }
  }

  @Test
  public void disabledPlaybackStaysPausedAfterResumeAndAfterADelayedHandoff() throws Exception {
    try (ActivityScenario<RustActivity> scene = RuntimeTest.launchOffline()) {
      PlaybackService service = start(false);
      scene.moveToState(Lifecycle.State.CREATED);
      main(() -> assertFalse(service.player.getPlayWhenReady()));
      // Simulate resolution finishing after onStop has already paused playback.
      main(
          () -> {
            try {
              service.play(new JSONObject(request.toString()));
            } catch (Exception error) {
              throw new AssertionError(error);
            }
            assertFalse(service.player.getPlayWhenReady());
          });
      scene.moveToState(Lifecycle.State.RESUMED);
      main(() -> assertFalse(service.player.getPlayWhenReady()));
    }
  }

  @Test
  public void lateBridgeHandoffIsRejectedBeforeStartingAForegroundService() throws Exception {
    try (ActivityScenario<RustActivity> scene = RuntimeTest.launchOffline()) {
      MobileSettings.save(new JSONObject().put("background_playback", false));
      scene.moveToState(Lifecycle.State.CREATED);
      MediaRuntimeTest.await(
          "Idle preparation closes", false, () -> PlaybackService.current == null);
      long token = 81726354;
      request.put("startup_token", token);
      MediaBridge.execute(new JSONObject().put("op", "play").put("request", request).toString());
      MediaRuntimeTest.await(
          "Late handoff reports cancellation",
          false,
          () ->
              MediaBridge.events.stream()
                  .anyMatch(
                      event ->
                          "playback_start_failed".equals(event.optString("kind"))
                              && Long.toString(token).equals(event.optString("request_token"))));
      main(() -> assertNull(PlaybackService.current));
    }
  }

  @Test
  public void previewTimerCannotRestartTheInterruptedItemWhenBackgroundPlaybackIsOff()
      throws Exception {
    try (ActivityScenario<RustActivity> scene = RuntimeTest.launchOffline()) {
      PlaybackService service = start(false);
      main(
          () -> {
            try {
              service.play(new JSONObject(request.toString()).put("preview", true));
            } catch (Exception error) {
              throw new AssertionError(error);
            }
          });
      scene.moveToState(Lifecycle.State.CREATED);
      main(
          () -> {
            service.finishPreview();
            assertFalse(service.currentRequest.optBoolean("preview"));
            assertFalse(service.player.getPlayWhenReady());
          });
    }
  }
}
