package dev.ytui.android;

import static org.junit.Assert.*;

import android.net.Uri;
import androidx.media3.datasource.DataSpec;
import androidx.media3.datasource.HttpDataSource;
import androidx.media3.exoplayer.offline.Download;
import androidx.media3.exoplayer.scheduler.Requirements;
import java.io.IOException;
import java.util.*;
import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;

@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 35)
@androidx.annotation.OptIn(markerClass = androidx.media3.common.util.UnstableApi.class)
public final class DownloadPolicyTest {
  private static final long MB = 1024L * 1024;

  private static IOException http(int code) {
    return new HttpDataSource.InvalidResponseCodeException(
        code,
        "status",
        null,
        Collections.emptyMap(),
        new DataSpec(Uri.parse("https://cdn.test/videoplayback")),
        new byte[0]);
  }

  @Test
  public void onlyExpiredSignedLinksAreRefreshed() {
    assertTrue(DownloadPolicy.expiredStream(http(403)));
    assertTrue(DownloadPolicy.expiredStream(http(410)));
    // The downloader may wrap the HTTP failure.
    assertTrue(DownloadPolicy.expiredStream(new IOException(new RuntimeException(http(403)))));
    assertEquals(403, DownloadPolicy.responseCode(new IOException(http(403))));
    for (int code : new int[] {404, 416, 429, 500})
      assertFalse(String.valueOf(code), DownloadPolicy.expiredStream(http(code)));
    assertFalse(DownloadPolicy.expiredStream(new IOException("connection reset")));
    assertFalse(DownloadPolicy.expiredStream(null));
    assertEquals(-1, DownloadPolicy.responseCode(null));
  }

  @Test
  public void anAutomaticallyRefreshedRequestIsNeverRefreshedAgain() throws Exception {
    JSONObject request = new JSONObject().put("media", new JSONObject().put("url", "u"));
    assertTrue(DownloadPolicy.autoRefreshAllowed(request));
    request.put(DownloadPolicy.AUTO_REFRESHED, true);
    assertFalse(DownloadPolicy.autoRefreshAllowed(request));
    assertFalse(DownloadPolicy.autoRefreshAllowed(null));
    // A manual retry clears the mark and allows one automatic refresh again.
    request.remove(DownloadPolicy.AUTO_REFRESHED);
    assertTrue(DownloadPolicy.autoRefreshAllowed(request));
  }

  @Test
  public void pairedAudioPartsMapToTheirDownload() {
    assertEquals("abc", DownloadPolicy.baseId("abc-audio"));
    assertEquals("abc", DownloadPolicy.baseId("abc"));
  }

  @Test
  public void requiredSpaceComesFromDeclaredSizesOrAFloor() throws Exception {
    JSONObject paired =
        new JSONObject()
            .put("url", "https://cdn.test/v?itag=137&clen=" + 300 * MB + "&lmt=1")
            .put("audio_url", "https://cdn.test/a?clen=" + 20 * MB);
    assertEquals(320 * MB + DownloadPolicy.RESERVE_BYTES, DownloadPolicy.requiredBytes(paired, 0));
    // Bytes already in the cache are not needed again.
    assertEquals(
        220 * MB + DownloadPolicy.RESERVE_BYTES, DownloadPolicy.requiredBytes(paired, 100 * MB));
    assertEquals(
        DownloadPolicy.RESERVE_BYTES, DownloadPolicy.requiredBytes(paired, 900 * MB));
    // Unknown audio size: fall back to the floor rather than under-estimating.
    JSONObject unknownAudio =
        new JSONObject(paired.toString()).put("audio_url", "https://cdn.test/a?itag=140");
    assertEquals(DownloadPolicy.UNKNOWN_SIZE_FLOOR, DownloadPolicy.requiredBytes(unknownAudio, 0));
    JSONObject single = new JSONObject().put("url", "https://cdn.test/v").put("audio_url", "null");
    assertEquals(DownloadPolicy.UNKNOWN_SIZE_FLOOR, DownloadPolicy.requiredBytes(single, 0));
    single.put("filesize", 10 * MB);
    assertEquals(10 * MB + DownloadPolicy.RESERVE_BYTES, DownloadPolicy.requiredBytes(single, 0));
    JSONObject approx = new JSONObject().put("url", "https://cdn.test/v?clen=bad");
    assertEquals(DownloadPolicy.UNKNOWN_SIZE_FLOOR, DownloadPolicy.requiredBytes(approx, 0));
    approx.put("filesize_approx", 5 * MB);
    assertEquals(5 * MB + DownloadPolicy.RESERVE_BYTES, DownloadPolicy.requiredBytes(approx, 0));
  }

  @Test
  public void lowSpaceRefusesWithAClearMessage() {
    assertTrue(DownloadPolicy.hasRoom(300 * MB, 300 * MB));
    assertFalse(DownloadPolicy.hasRoom(150 * MB, DownloadPolicy.UNKNOWN_SIZE_FLOOR));
    String message = DownloadPolicy.spaceError(150 * MB, 384 * MB);
    assertTrue(message, message.startsWith("Not enough free space"));
    assertTrue(message, message.contains("384 MB") && message.contains("150 MB free"));
  }

  @Test
  public void wifiOnlySelectsAnUnmeteredNetwork() {
    assertEquals(Requirements.NETWORK_UNMETERED, DownloadPolicy.networkRequirement(true));
    assertEquals(Requirements.NETWORK, DownloadPolicy.networkRequirement(false));
  }

  @Test
  public void notificationCountsEachVideoOnceAndAggregatesProgress() {
    List<DownloadPolicy.Part> parts =
        Arrays.asList(
            new DownloadPolicy.Part("a", Download.STATE_DOWNLOADING, 50, 100, 50f),
            new DownloadPolicy.Part("a-audio", Download.STATE_DOWNLOADING, 50, 100, 50f),
            new DownloadPolicy.Part("b", Download.STATE_QUEUED, 0, 200, 0f),
            new DownloadPolicy.Part("c", Download.STATE_COMPLETED, 10, 10, 100f),
            // Video finished, its audio is still running: still one visible download.
            new DownloadPolicy.Part("d-audio", Download.STATE_DOWNLOADING, 100, 100, 100f));
    DownloadPolicy.Summary summary = DownloadPolicy.summarize(parts);
    assertEquals(3, summary.items);
    assertEquals(2, summary.downloading);
    assertEquals(40, summary.percent); // 200 of 500 bytes
    assertEquals("3 downloads · 40%", DownloadPolicy.notificationText(summary, 0));
    assertEquals(
        "3 downloads · waiting for Wi-Fi",
        DownloadPolicy.notificationText(summary, Requirements.NETWORK_UNMETERED));
    assertEquals(
        "3 downloads · waiting for a network",
        DownloadPolicy.notificationText(summary, Requirements.NETWORK));
  }

  @Test
  public void notificationProgressWithoutLengthsUsesPartPercentages() {
    DownloadPolicy.Summary unknown =
        DownloadPolicy.summarize(
            Arrays.asList(
                new DownloadPolicy.Part("a", Download.STATE_DOWNLOADING, 5, -1, 20f),
                new DownloadPolicy.Part("b", Download.STATE_DOWNLOADING, 5, 100, 60f)));
    assertEquals(40, unknown.percent);
    DownloadPolicy.Summary noPercent =
        DownloadPolicy.summarize(
            Collections.singletonList(
                new DownloadPolicy.Part("a", Download.STATE_DOWNLOADING, 0, -1, -1f)));
    assertEquals(-1, noPercent.percent);
    assertEquals("1 download", DownloadPolicy.notificationText(noPercent, 0));
    DownloadPolicy.Summary queued =
        DownloadPolicy.summarize(
            Collections.singletonList(
                new DownloadPolicy.Part("a", Download.STATE_QUEUED, 0, 100, 0f)));
    assertEquals(-1, queued.percent);
    assertEquals("1 download queued", DownloadPolicy.notificationText(queued, 0));
    assertEquals(
        "Finishing downloads",
        DownloadPolicy.notificationText(DownloadPolicy.summarize(Collections.emptyList()), 0));
  }
}
