package dev.ytui.android;

import java.util.ArrayList;
import java.util.Collections;
import org.json.*;

/** In-memory source identity only; never persisted or logged (may contain stream leases). */
final class SourcePreparationKey {
  static String of(JSONObject request, long account, long network) throws JSONException {
    JSONObject stream = request.getJSONObject("stream");
    JSONObject selected = new JSONObject();
    for (String field : new String[] {"url", "audio_url", "dash_manifest", "audio_dash_manifest",
        "hls_master", "live", "headers", "cache_key", "audio_cache_key", "catalog"})
      if (stream.has(field)) selected.put(field, stream.get(field));
    return canonical(new JSONObject()
        .put("media", request.getJSONObject("media"))
        .put("stream", selected)
        .put("quality", request.optString("quality", "best"))
        .put("audio", request.optBoolean("audio"))
        .put("account", account).put("network", network));
  }

  private static String canonical(Object value) throws JSONException {
    if (value instanceof JSONObject) {
      JSONObject object = (JSONObject) value;
      ArrayList<String> keys = new ArrayList<>();
      object.keys().forEachRemaining(keys::add);
      Collections.sort(keys);
      StringBuilder result = new StringBuilder("{");
      for (String key : keys) {
        if (result.length() > 1) result.append(',');
        result.append(JSONObject.quote(key)).append(':').append(canonical(object.get(key)));
      }
      return result.append('}').toString();
    }
    if (value instanceof JSONArray) {
      JSONArray array = (JSONArray) value;
      StringBuilder result = new StringBuilder("[");
      for (int i = 0; i < array.length(); i++) {
        if (i > 0) result.append(',');
        result.append(canonical(array.get(i)));
      }
      return result.append(']').toString();
    }
    return value instanceof String ? JSONObject.quote((String) value) : String.valueOf(value);
  }

  private SourcePreparationKey() {}
}
