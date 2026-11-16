package dev.ytui.android;

import static org.junit.Assert.*;
import org.json.*;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;

@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 35)
public final class SourcePreparationKeyTest {
  private JSONObject request() throws Exception {
    return new JSONObject("{\"media\":{\"url\":\"https://example.invalid/item\",\"title\":\"title\"},"
        + "\"stream\":{\"url\":\"https://example.invalid/stream\",\"headers\":{\"A\":\"1\",\"B\":\"2\"}},"
        + "\"audio\":false,\"quality\":\"1080\"}");
  }

  @Test public void scopesAndSelectedResourcesCannotCross() throws Exception {
    String key = SourcePreparationKey.of(request(), 1, 2);
    assertNotEquals(key, SourcePreparationKey.of(request(), 2, 2));
    assertNotEquals(key, SourcePreparationKey.of(request(), 1, 3));
    for (String field : new String[] {"url", "audio_url", "dash_manifest", "audio_dash_manifest",
        "hls_master", "cache_key", "audio_cache_key", "catalog", "headers"}) {
      JSONObject request = request();
      request.getJSONObject("stream").put(field, "different");
      assertNotEquals(field, key, SourcePreparationKey.of(request, 1, 2));
    }
    assertNotEquals(key, SourcePreparationKey.of(request().put("audio", true), 1, 2));
    assertNotEquals(key, SourcePreparationKey.of(request().put("quality", "720"), 1, 2));
    JSONObject renamed = request();
    renamed.getJSONObject("media").put("title", "new title");
    assertNotEquals(key, SourcePreparationKey.of(renamed, 1, 2));
  }

  @Test public void jsonOrderAndResolutionDiagnosticsDoNotPreventReuse() throws Exception {
    JSONObject request = request();
    String key = SourcePreparationKey.of(request, 1, 2);
    request.getJSONObject("stream").put("headers", new JSONObject("{\"B\":\"2\",\"A\":\"1\"}"));
    request.getJSONObject("stream").put("resolution_cache_hit", true).put("extract_ms", 99);
    assertEquals(key, SourcePreparationKey.of(request, 1, 2));
  }
}
