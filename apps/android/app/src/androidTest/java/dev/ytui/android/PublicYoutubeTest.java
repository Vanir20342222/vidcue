package dev.ytui.android;

import static org.junit.Assert.*;

import androidx.test.core.app.ActivityScenario;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import androidx.test.platform.app.InstrumentationRegistry;
import org.json.*;
import org.junit.*;
import org.junit.runner.RunWith;

/** Explicit public-only acceptance; never imports an account or prints a stream URL. */
@RunWith(AndroidJUnit4.class)
public final class PublicYoutubeTest {
  @org.junit.Rule
  public androidx.test.rule.GrantPermissionRule notifications =
      androidx.test.rule.GrantPermissionRule.grant(
          android.os.Build.VERSION.SDK_INT >= 33
              ? new String[] {"android.permission.POST_NOTIFICATIONS"}
              : new String[] {});

  @Test
  public void nativePublicYoutubeActuallyDecodesWhenExplicitlyRequested() throws Exception {
    String url = InstrumentationRegistry.getArguments().getString("youtubeUrl", "");
    Assume.assumeTrue("Public network check is opt-in", !url.isEmpty());
    android.net.Uri parsed = android.net.Uri.parse(url);
    String host = parsed.getHost();
    assertTrue(
        "Use a public YouTube HTTP(S) URL",
        ("http".equals(parsed.getScheme()) || "https".equals(parsed.getScheme()))
            && host != null
            && (host.equals("youtu.be")
                || host.equals("youtube.com")
                || host.endsWith(".youtube.com")));
    String quality = InstrumentationRegistry.getArguments().getString("youtubeQuality", "720");
    int budget =
        Integer.parseInt(
            InstrumentationRegistry.getArguments().getString("youtubeTargetMs", "2000"));
    assertTrue(
        "Supported public quality",
        java.util.Arrays.asList("best", "1080", "720").contains(quality));
    assertTrue("Supported public budget", budget == 1000 || budget == 2000);
    try (ActivityScenario<RustActivity> activity = RuntimeTest.launchOffline()) {
      PlaybackCache.cancel();
      NativeBridge.request("{\"op\":\"resolve_cache_clear\"}");
      JSONObject media =
          new JSONObject()
              .put("url", url)
              .put("title", "Public YouTube acceptance")
              .put("kind", "video");
      long timing = StartupTimings.begin(android.os.SystemClock.elapsedRealtime(), true);
      StartupTimings.byteCache(timing, false);
      boolean passed = false;
      try {
        JSONObject resolved =
            new JSONObject(
                NativeBridge.request(
                    new JSONObject()
                        .put("op", "resolve")
                        .put("media", media)
                        .put("audio", false)
                        .put("quality", quality)
                        .toString()));
        assertTrue(
            "Native public extraction: " + resolved.optString("error"), resolved.optBoolean("ok"));
        JSONObject stream = resolved.getJSONObject("data");
        MediaBridge.recordResolutionTimings(timing, stream);
        StartupTimings.resolved(
            timing,
            android.os.SystemClock.elapsedRealtime(),
            stream.optBoolean("resolution_cache_hit"));
        StartupTimings.route(timing, stream.optString("resolution_route", "direct-media"));
        JSONObject request =
            new JSONObject()
                .put("media", media)
                .put("startup_token", timing)
                .put("stream", stream)
                .put("quality", quality)
                .put("audio", false);
        MediaBridge.execute(new JSONObject().put("op", "play").put("request", request).toString());
        MediaBridge.event("show_player", "");
        MediaRuntimeTest.await(
            "Public YouTube playback did not decode three seconds",
            () ->
                MediaRuntimeTest.status().optDouble("position") > 3
                    && MediaRuntimeTest.status().optBoolean("playing"));
        assertTrue(MediaRuntimeTest.status().isNull("error"));
        MediaRuntimeTest.await(
            "Public YouTube never reached visible and active output",
            () -> StartupTimings.usable(timing) > 0);
        long usable = StartupTimings.usable(timing);
        assertTrue(
            "Live visible+playing target missed: " + StartupTimings.report(),
            budget == 1000 ? usable < 1000 : usable <= 2000);
        passed = true;
      } finally {
        if (!passed)
          StartupTimings.ended(timing, "failed", android.os.SystemClock.elapsedRealtime());
        try {
          java.io.File report =
              new java.io.File(
                  MediaBridge.context.getExternalFilesDir(null), "public-startup-timings.txt");
          java.nio.file.Files.write(
              report.toPath(),
              ("public_test_passed="
                      + passed
                      + " quality="
                      + quality
                      + " budget_ms="
                      + budget
                      + "\n"
                      + StartupTimings.report())
                  .getBytes(java.nio.charset.StandardCharsets.UTF_8));
        } finally {
          MediaRuntimeTest.command("stop");
        }
      }
    }
  }
}
