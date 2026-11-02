package dev.ytui.android;

import static org.junit.Assert.*;

import java.net.Proxy;
import java.util.concurrent.TimeUnit;
import okhttp3.*;
import okhttp3.mockwebserver.*;
import org.junit.Test;

public final class PlaybackHttpTest {
  @Test
  public void aClosedPooledConnectionRecoversWithoutRetryingRateLimits() throws Exception {
    OkHttpClient client = PlaybackHttp.newClient(null).proxy(Proxy.NO_PROXY).build();
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setBody("first"));
      server.enqueue(new MockResponse().setSocketPolicy(SocketPolicy.DISCONNECT_AFTER_REQUEST));
      server.enqueue(new MockResponse().setBody("recovered"));
      server.enqueue(new MockResponse().setResponseCode(429).addHeader("Retry-After", "30"));
      server.start();
      try (Response first =
          client.newCall(new Request.Builder().url(server.url("/first")).build()).execute()) {
        assertEquals("first", first.body().string());
      }
      try (Response next =
          client.newCall(new Request.Builder().url(server.url("/next")).build()).execute()) {
        assertEquals("recovered", next.body().string());
      }
      assertEquals(3, server.getRequestCount());
      assertEquals(0, server.takeRequest(2, TimeUnit.SECONDS).getSequenceNumber());
      assertEquals(
          "Failure must occur on the reused socket",
          1,
          server.takeRequest(2, TimeUnit.SECONDS).getSequenceNumber());
      assertEquals(
          "Recovery must use a fresh connection",
          0,
          server.takeRequest(2, TimeUnit.SECONDS).getSequenceNumber());
      try (Response limited =
          client.newCall(new Request.Builder().url(server.url("/limited")).build()).execute()) {
        assertEquals(429, limited.code());
      }
      assertNotNull(server.takeRequest(2, TimeUnit.SECONDS));
      assertNull(
          "HTTP429 must never trigger another request",
          server.takeRequest(200, TimeUnit.MILLISECONDS));
      assertEquals(4, server.getRequestCount());
    } finally {
      client.connectionPool().evictAll();
      client.dispatcher().executorService().shutdownNow();
    }
  }
}
