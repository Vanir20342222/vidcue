package dev.ytui.android;

import static org.junit.Assert.*;

import org.json.JSONObject;
import org.junit.Before;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.RuntimeEnvironment;
import org.robolectric.annotation.Config;

@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 35)
public final class PlaybackRecoveryTest {
  private static final long NOW = 1_800_000_000_000L;

  @Before
  public void setup() {
    MediaBridge.context = RuntimeEnvironment.getApplication();
    PlaybackRecovery.clear();
  }

  private JSONObject request() throws Exception {
    return new JSONObject()
        .put(
            "media",
            new JSONObject()
                .put("url", "https://music.youtube.com/watch?v=abcdefghijk&list=private&sig=secret")
                .put("id", "abcdefghijk")
                .put("kind", "song")
                .put("title", "Synthetic song")
                .put("creator", "Fixture")
                .put("thumbnail", "https://art.invalid/cover?token=secret")
                .put("unknown_private_field", "secret"))
        .put(
            "stream",
            new JSONObject()
                .put("url", "https://decoder.invalid/?sig=secret")
                .put("headers", new JSONObject().put("Cookie", "secret")))
        .put("context", new JSONObject().put("private", "secret"))
        .put("audio", true)
        .put("quality", "720");
  }

  @Test
  public void checkpointRetainsPositionAndPublicIdentityWithoutSessionSecrets() throws Exception {
    JSONObject saved = PlaybackRecovery.encode(request(), 12_345, 60_000, NOW);
    assertEquals(12.345, saved.getDouble("position"), 0.0001);
    assertEquals(60, saved.getDouble("duration"), 0);
    assertEquals("720", saved.getString("quality"));
    assertEquals(
        "https://www.youtube.com/watch?v=abcdefghijk",
        saved.getJSONObject("media").getString("url"));
    assertFalse(saved.toString().contains("secret"));
    assertFalse(saved.has("stream"));
    assertFalse(saved.has("context"));
    JSONObject restored = PlaybackRecovery.decode(saved.toString(), NOW + 1000);
    assertTrue(restored.getBoolean("recovered"));
    assertTrue(restored.getBoolean("paused"));
    assertFalse(restored.getBoolean("playing"));
    assertFalse(restored.getBoolean("video"));
  }

  @Test
  public void malformedExpiredFutureOrUnknownVersionCannotRestore() throws Exception {
    assertNull(PlaybackRecovery.decode("{", NOW));
    assertNull(PlaybackRecovery.decode("x".repeat(8193), NOW));
    JSONObject saved = PlaybackRecovery.encode(request(), 0, 0, NOW);
    assertNull(PlaybackRecovery.decode(saved.toString(), NOW + PlaybackRecovery.MAX_AGE_MS + 1));
    assertNull(PlaybackRecovery.decode(saved.toString(), NOW - 1));
    assertNull(PlaybackRecovery.decode(saved.put("version", 2).toString(), NOW));
  }

  @Test
  public void previewsAndSignedDirectLinksAreNotRecoveryCandidates() throws Exception {
    assertNull(PlaybackRecovery.encode(request().put("preview", true), 0, 0, NOW));
    JSONObject request = request();
    request.getJSONObject("media").put("url", "https://media.invalid/audio?token=secret");
    assertNull(PlaybackRecovery.encode(request, 0, 0, NOW));
    request.getJSONObject("media").put("url", "https://user:password@media.invalid/audio");
    assertNull(PlaybackRecovery.encode(request, 0, 0, NOW));
  }

  @Test
  public void offlineMediaRetainsOnlyItsLibraryIdentity() throws Exception {
    JSONObject request = request();
    request
        .getJSONObject("media")
        .put("url", "content://private/item?token=secret")
        .put("offline_id", "fixture-download");
    JSONObject saved = PlaybackRecovery.encode(request, 2000, 6000, NOW);
    assertEquals("fixture-download", saved.getJSONObject("media").getString("offline_id"));
    assertEquals("offline:recovery", saved.getJSONObject("media").getString("url"));
    assertNotNull(PlaybackRecovery.decode(saved.toString(), NOW));
    assertFalse(saved.toString().contains("secret"));
  }

  @Test
  public void radioRestartsAtLiveEdgeAndPositionsAreClamped() throws Exception {
    JSONObject request = request();
    request.getJSONObject("media").put("live", true).put("url", "https://radio.invalid/stream");
    assertEquals(0, PlaybackRecovery.encode(request, 12345, -1, NOW).getDouble("position"), 0);
    request.getJSONObject("media").put("live", false);
    assertEquals(0, PlaybackRecovery.encode(request, -2, 1000, NOW).getDouble("position"), 0);
    assertEquals(1, PlaybackRecovery.encode(request, 3000, 1000, NOW).getDouble("position"), 0);
  }

  @Test
  public void storeCanReloadSeekAndClearWithoutAService() throws Exception {
    PlaybackRecovery.save(request(), 12000, 60000);
    assertEquals(12, PlaybackRecovery.snapshot().getDouble("position"), 0);
    PlaybackRecovery.seek(23000);
    assertEquals(23, PlaybackRecovery.snapshot().getDouble("position"), 0);
    PlaybackRecovery.clear();
    assertNull(PlaybackRecovery.snapshot());
  }

  @Test
  public void unsupportedReplacementDoesNotResurrectThePreviousItem() throws Exception {
    PlaybackRecovery.save(request(), 12000, 60000);
    PlaybackRecovery.save(new JSONObject(), 0, 0);
    assertNull(PlaybackRecovery.snapshot());
  }
}
