package dev.ytui.android;

import android.content.Context;
import android.content.SharedPreferences;
import java.net.URI;
import org.json.JSONObject;

/** Small app-private checkpoint; decoder URLs, headers and account/context data stay in memory. */
final class PlaybackRecovery {
  private static final String KEY = "checkpoint";
  static final long MAX_AGE_MS = 7L * 24 * 60 * 60 * 1000;

  private static SharedPreferences prefs() {
    return MediaBridge.context.getSharedPreferences("playback-recovery", Context.MODE_PRIVATE);
  }

  static void save(JSONObject request, long position, long duration) {
    JSONObject value = encode(request, position, duration, System.currentTimeMillis());
    // Replacing unsupported media must not leave the previous item as a recovery candidate.
    if (value == null) clear();
    else prefs().edit().putString(KEY, value.toString()).apply();
  }

  static void clear() {
    prefs().edit().remove(KEY).apply();
  }

  static JSONObject snapshot() {
    return decode(prefs().getString(KEY, ""), System.currentTimeMillis());
  }

  static void seek(long position) {
    JSONObject value = snapshot();
    if (value == null) return;
    try {
      value.put("position", Math.max(0, Math.min(position / 1000.0, value.optDouble("duration"))));
      prefs().edit().putString(KEY, value.toString()).apply();
      MediaBridge.notifyUi();
    } catch (Exception ignored) {
    }
  }

  static JSONObject encode(JSONObject request, long position, long duration, long now) {
    if (request.optBoolean("preview")) return null;
    JSONObject source = request.optJSONObject("media");
    if (source == null) return null;
    try {
      String offline = bounded(source.optString("offline_id", ""), 2048);
      String url = recoverableUrl(source.optString("url", ""));
      if (url.isEmpty() && offline.isEmpty()) return null;
      JSONObject media = new JSONObject();
      for (String key : new String[] {"id", "title", "creator", "kind"})
        media.put(key, bounded(source.optString(key, ""), 512));
      media.put("url", url.isEmpty() ? "offline:recovery" : url);
      media.put("live", source.optBoolean("live"));
      if (!offline.isEmpty()) media.put("offline_id", offline);
      long length = Math.max(0, duration);
      long at = source.optBoolean("live") ? 0 : Math.max(0, position);
      if (length > 0) at = Math.min(at, length);
      return new JSONObject()
          .put("version", 1)
          .put("saved_at", now)
          .put("recovered", true)
          .put("media", media)
          .put("title", media.optString("title"))
          .put("video", !request.optBoolean("audio"))
          .put("quality", bounded(request.optString("quality", "1080"), 32))
          .put("position", at / 1000.0)
          .put("duration", length / 1000.0)
          .put("playing", false)
          .put("paused", true);
    } catch (Exception ignored) {
      return null;
    }
  }

  static JSONObject decode(String raw, long now) {
    if (raw == null || raw.length() > 8192) return null;
    try {
      JSONObject value = new JSONObject(raw);
      long age = now - value.getLong("saved_at");
      if (value.optInt("version") != 1 || age < 0 || age > MAX_AGE_MS) return null;
      // Rebuild from the allowlist, also validating malformed/old persisted data.
      JSONObject request =
          new JSONObject()
              .put("media", value.getJSONObject("media"))
              .put("audio", !value.optBoolean("video"))
              .put("quality", value.optString("quality", "1080"));
      double position = value.optDouble("position", 0), duration = value.optDouble("duration", 0);
      if (!Double.isFinite(position) || !Double.isFinite(duration)) return null;
      return encode(request, (long) (position * 1000), (long) (duration * 1000), now);
    } catch (Exception ignored) {
      return null;
    }
  }

  private static String recoverableUrl(String raw) {
    if (raw.length() > 2048) return "";
    try {
      URI uri = new URI(raw);
      if (!("https".equals(uri.getScheme()) || "http".equals(uri.getScheme()))
          || uri.getHost() == null
          || uri.getRawUserInfo() != null
          || uri.getRawFragment() != null) return "";
      String host = uri.getHost();
      if (host.equals("youtube.com")
          || host.equals("www.youtube.com")
          || host.equals("music.youtube.com")
          || host.equals("youtu.be")) {
        String id = host.equals("youtu.be") ? uri.getPath().substring(1) : "";
        if (!host.equals("youtu.be") && "/watch".equals(uri.getPath()) && uri.getRawQuery() != null)
          for (String pair : uri.getRawQuery().split("&"))
            if (pair.startsWith("v=")) id = pair.substring(2);
        return id.matches("[A-Za-z0-9_-]{11}") ? "https://www.youtube.com/watch?v=" + id : "";
      }
      // Radio/direct public media can be re-resolved, but never retain signed query parameters.
      return uri.getRawQuery() == null ? raw : "";
    } catch (Exception ignored) {
      return "";
    }
  }

  private static String bounded(String text, int limit) {
    return text.length() <= limit ? text : text.substring(0, limit);
  }

  private PlaybackRecovery() {}
}
