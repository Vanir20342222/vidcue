package dev.ytui.android;

import androidx.media3.datasource.HttpDataSource;
import androidx.media3.datasource.okhttp.OkHttpDataSource;
import java.net.ProxySelector;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.TimeUnit;
import okhttp3.Call;
import okhttp3.ConnectionPool;
import okhttp3.CookieJar;
import okhttp3.OkHttpClient;

/** One connection pool per routing/account epoch; headers always belong to the request factory. */
@androidx.annotation.OptIn(markerClass = androidx.media3.common.util.UnstableApi.class)
final class PlaybackHttp {
  private static OkHttpClient shared;
  private static long network = -1, account = -1;

  static synchronized OkHttpClient client() {
    long nextNetwork = NetworkSettings.revision.get();
    long nextAccount = NativeBridge.accountIntent();
    if (shared == null || network != nextNetwork || account != nextAccount) {
      if (shared != null) shared.connectionPool().evictAll();
      shared = newClient(ProxySelector.getDefault()).build();
      network = nextNetwork;
      account = nextAccount;
    }
    return shared;
  }

  static OkHttpClient.Builder newClient(ProxySelector proxy) {
    OkHttpClient.Builder builder =
        new OkHttpClient.Builder()
            .connectionPool(new ConnectionPool(8, 5, TimeUnit.MINUTES))
            .connectTimeout(8, TimeUnit.SECONDS)
            .readTimeout(12, TimeUnit.SECONDS)
            .followRedirects(true)
            .followSslRedirects(false)
            .retryOnConnectionFailure(true)
            .cookieJar(CookieJar.NO_COOKIES);
    if (proxy != null) builder.proxySelector(proxy);
    return builder;
  }

  static HttpDataSource.Factory factory(Map<String, String> headers) {
    return factory(headers, null);
  }

  static HttpDataSource.Factory factory(Map<String, String> headers, Cancellation cancellation) {
    // A MediaSource may outlive a settings change. Select the current route on
    // every new request, not just when its factory was first constructed.
    Call.Factory calls =
        request -> {
          Call call = client().newCall(request);
          return cancellation == null ? call : cancellation.track(call);
        };
    return new OkHttpDataSource.Factory(calls).setDefaultRequestProperties(headers);
  }

  static void warm() {
    java.util.concurrent.ForkJoinPool.commonPool()
        .execute(
            () -> {
              try {
                client();
              } catch (RuntimeException unavailable) {
                // Optional class/TLS initialization; foreground playback still reports errors.
              }
            });
  }

  /** Cancel sockets as well as CacheWriter flags, so a tap immediately releases preload I/O. */
  static final class Cancellation {
    private final Set<Call> calls = ConcurrentHashMap.newKeySet();
    private volatile boolean cancelled;

    Call track(Call call) {
      calls.add(call);
      if (cancelled) call.cancel();
      return call;
    }

    void cancel() {
      cancelled = true;
      for (Call call : calls) call.cancel();
    }

    void close() {
      cancel();
      calls.clear();
    }
  }

  private PlaybackHttp() {}
}
