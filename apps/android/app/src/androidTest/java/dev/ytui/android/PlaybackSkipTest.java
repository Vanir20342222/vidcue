package dev.ytui.android;

import static org.junit.Assert.*;

import androidx.test.core.app.ActivityScenario;
import androidx.test.platform.app.InstrumentationRegistry;
import java.io.InputStream;
import java.io.ByteArrayOutputStream;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import org.json.*;
import org.junit.*;

/** Deliberate Next intent must survive a decoder that never starts and resolution failure. */
public final class PlaybackSkipTest {
  @Rule public androidx.test.rule.GrantPermissionRule notifications =
      androidx.test.rule.GrantPermissionRule.grant(
          android.os.Build.VERSION.SDK_INT >= 33
              ? new String[] {"android.permission.POST_NOTIFICATIONS"} : new String[] {});

  @Test public void repeatedNextDoesNotWaitForTheIntermediateDecoder() throws Exception {
    runSkip(false);
  }

  @Test public void nextCanSkipAnItemWhoseResolutionFailed() throws Exception {
    runSkip(true);
  }

  private void runSkip(boolean resolutionFailure) throws Exception {
    JSONObject original = PlaybackService.rpc(new JSONObject().put("op", "settings"));
    CountDownLatch releaseNavigation = new CountDownLatch(1);
    byte[] audio;
    try (InputStream input = InstrumentationRegistry.getInstrumentation().getContext()
        .getAssets().open("fixture-audio.m4a")) {
      ByteArrayOutputStream bytes = new ByteArrayOutputStream();
      byte[] buffer = new byte[8192];
      int count;
      while ((count = input.read(buffer)) != -1) bytes.write(buffer, 0, count);
      audio = bytes.toByteArray();
    }
    try (ActivityScenario<RustActivity> activity = RuntimeTest.launchOffline();
        PlaybackNavigationTest.CollectionServer server =
            new PlaybackNavigationTest.CollectionServer(audio)) {
      PlaybackNavigationTest.clearQueue();
      PlaybackService.rpc(new JSONObject().put("op", "save_settings").put("config",
          new JSONObject(original.toString()).put("proxy", JSONObject.NULL).put("playback_history", true)));
      MediaBridge.playbackStore.submit(() -> {
        PlaybackService.rpcArray(new JSONObject().put("op", "history_clear"));
        return null;
      }).get(5, TimeUnit.SECONDS);
      server.pauseMedia = new CountDownLatch(1);
      JSONObject first = PlaybackNavigationTest.media(server, "First");
      MediaBridge.execute(new JSONObject().put("op", "play").put("request",
          new JSONObject().put("media", first).put("audio", true)
              .put("stream", new JSONObject().put("url", first.getString("url")))).toString());
      PlaybackNavigationTest.playing("First");
      MediaRuntimeTest.command("pause");
      MediaRuntimeTest.await("Initial item did not pause", () -> MediaRuntimeTest.status().optBoolean("paused"));
      JSONObject middle = PlaybackNavigationTest.media(server, "Buffering");
      if (resolutionFailure)
        middle.put("url", server.url("/Unresolvable.m3u"));
      PlaybackService.rpc(new JSONObject().put("op", "enqueue").put("media", middle));
      PlaybackService.rpc(new JSONObject().put("op", "enqueue")
          .put("media", PlaybackNavigationTest.media(server, "Recovery")));
      if (resolutionFailure) {
        MediaRuntimeTest.command("next");
        MediaRuntimeTest.await("Resolution failure was not shown",
            () -> !MediaRuntimeTest.status().isNull("error"));
        assertEquals(2, PlaybackService.rpcArray(new JSONObject().put("op", "queue")).length());
        MediaRuntimeTest.command("next");
      } else {
        // Hold the ordered persistence lane so both taps precede resolution.
        CountDownLatch held = new CountDownLatch(1);
        MediaBridge.playbackStore.execute(() -> {
          held.countDown();
          try { releaseNavigation.await(10, TimeUnit.SECONDS); }
          catch (InterruptedException e) { Thread.currentThread().interrupt(); }
        });
        assertTrue(held.await(5, TimeUnit.SECONDS));
        InstrumentationRegistry.getInstrumentation().runOnMainSync(() -> {
          PlaybackService.current.next();
          PlaybackService.current.next();
        });
        releaseNavigation.countDown();
      }
      PlaybackNavigationTest.playing("Recovery");
      assertEquals("Next must not wait for Buffering to start", 1, server.pauseMedia.getCount());
      MediaBridge.playbackStore.submit(() -> null).get(5, TimeUnit.SECONDS);
      assertEquals(0, PlaybackService.rpcArray(new JSONObject().put("op", "queue")).length());
      JSONArray history = PlaybackService.rpcArray(new JSONObject().put("op", "history_list"));
      assertEquals("Only First and Recovery should be recorded", 2, history.length());
      for (int i = 0; i < history.length(); i++)
        assertNotEquals("Buffering", history.getJSONObject(i).getString("title"));
    } finally {
      releaseNavigation.countDown();
      MediaRuntimeTest.command("stop");
      MediaBridge.playbackStore.submit(() -> null).get(5, TimeUnit.SECONDS);
      PlaybackNavigationTest.clearQueue();
      PlaybackService.rpc(new JSONObject().put("op", "save_settings").put("config", original));
    }
  }
}
