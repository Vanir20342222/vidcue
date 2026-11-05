package dev.ytui.android;

import android.content.Context;
import android.graphics.Bitmap;
import android.net.Uri;
import androidx.annotation.Nullable;
import androidx.media3.common.util.BitmapLoader;
import androidx.media3.datasource.DataSourceBitmapLoader;
import com.google.common.util.concurrent.ListenableFuture;
import com.google.common.util.concurrent.ListeningExecutorService;
import com.google.common.util.concurrent.MoreExecutors;
import java.util.concurrent.Executors;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * Cover art for the media notification, lock screen and system media controls.
 *
 * <p>The session's metadata names the cover by its public URL ({@link #coverUri}), so other
 * controllers that read the URI still find a real image. The session itself decodes the cover
 * through the Rust image cache the browse grid already filled, which also covers downloads and
 * account-only artwork, and falls back to YouTube's standard image like the in-app cover does.
 */
@androidx.annotation.OptIn(markerClass = androidx.media3.common.util.UnstableApi.class)
final class SessionArtwork implements BitmapLoader {
  /** Notification and lock-screen artwork are drawn well below this; it is also a cached size. */
  static final int PIXELS = 480;

  private final BitmapLoader network;
  private final ListeningExecutorService decoder =
      MoreExecutors.listeningDecorator(
          Executors.newSingleThreadExecutor(
              task -> {
                Thread thread = new Thread(task, "vidcue-session-artwork");
                thread.setDaemon(true);
                return thread;
              }));
  @Nullable private volatile Uri registeredUri;
  @Nullable private volatile String registeredMedia;

  SessionArtwork(Context context) {
    network = new DataSourceBitmapLoader(context);
  }

  /**
   * The public URL that identifies [media]'s cover, or null when it has none. Prefers the largest
   * listed variant, then the single thumbnail, then YouTube's standard image for a watch URL.
   */
  @Nullable
  static String coverUri(@Nullable JSONObject media) {
    if (media == null) return null;
    JSONArray variants = media.optJSONArray("thumbnails");
    String best = null;
    long bestArea = -1;
    if (variants != null) {
      for (int i = 0; i < variants.length(); i++) {
        JSONObject variant = variants.optJSONObject(i);
        String url = variant == null ? "" : variant.optString("url");
        if (!usable(url)) continue;
        long area = Math.max(0, variant.optLong("width")) * Math.max(0, variant.optLong("height"));
        if (area > bestArea) {
          best = url;
          bestArea = area;
        }
      }
    }
    if (best != null) return best;
    String thumbnail = media.optString("thumbnail");
    if (usable(thumbnail)) return thumbnail;
    return ArtworkSourceKt.youtubeArtworkFallback(
        media.optString("url"), media.optString("kind", "video"), "", false);
  }

  private static boolean usable(@Nullable String url) {
    return url != null && !url.isEmpty() && !url.equals("null");
  }

  /**
   * Names the cover the session should show next and returns its URI for the metadata. Only the
   * current item is kept: the session asks for the cover of the item it is about to display.
   */
  @Nullable
  Uri register(@Nullable JSONObject media) {
    String cover = coverUri(media);
    if (cover == null) {
      registeredUri = null;
      registeredMedia = null;
      return null;
    }
    Uri uri = Uri.parse(cover);
    registeredMedia = media.toString();
    registeredUri = uri;
    return uri;
  }

  @Override
  public boolean supportsMimeType(String mimeType) {
    return network.supportsMimeType(mimeType);
  }

  @Override
  public ListenableFuture<Bitmap> decodeBitmap(byte[] data) {
    return network.decodeBitmap(data);
  }

  @Override
  public ListenableFuture<Bitmap> loadBitmap(Uri uri) {
    String media = uri.equals(registeredUri) ? registeredMedia : null;
    if (media == null) return network.loadBitmap(uri);
    return decoder.submit(
        () -> {
          Bitmap cover = decode(media);
          if (cover == null) {
            // A listed cover (for example a max-resolution one) may not exist for every upload.
            String standard =
                ArtworkSourceKt.youtubeArtworkFallback(
                    new JSONObject(media).optString("url"), "video", "", false);
            if (standard != null && !standard.equals(uri.toString()))
              cover =
                  decode(
                      new JSONObject(media)
                          .put("thumbnail", standard)
                          .put("thumbnails", new JSONArray())
                          .toString());
          }
          if (cover == null) throw new IllegalStateException("Cover not available");
          return cover;
        });
  }

  @Nullable
  private static Bitmap decode(String media) {
    int[] data = NativeBridge.thumbnail(media, PIXELS);
    if (data == null
        || data.length < 3
        || data[0] < 1
        || data[0] > 768
        || data[1] < 1
        || data[1] > 768
        || data.length != data[0] * data[1] + 2) return null;
    return Bitmap.createBitmap(data, 2, data[0], data[0], data[1], Bitmap.Config.ARGB_8888);
  }

  void release() {
    decoder.shutdownNow();
  }
}
