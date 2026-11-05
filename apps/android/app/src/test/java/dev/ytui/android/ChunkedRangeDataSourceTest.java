package dev.ytui.android;

import static org.junit.Assert.*;

import android.net.Uri;
import androidx.media3.common.C;
import androidx.media3.datasource.*;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicInteger;
import org.junit.After;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;

@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 35)
@androidx.annotation.OptIn(markerClass = androidx.media3.common.util.UnstableApi.class)
public final class ChunkedRangeDataSourceTest {
  private static final String CDN = "https://r1.googlevideo.com/videoplayback?id=1";
  private final ExecutorService pool = Executors.newCachedThreadPool();

  @After
  public void stop() {
    pool.shutdownNow();
  }

  /** A CDN whose connections serve exactly the requested slice. */
  private static final class Cdn implements DataSource.Factory {
    final byte[] body;
    final List<String> opens = Collections.synchronizedList(new ArrayList<>());
    final AtomicInteger live = new AtomicInteger(), peak = new AtomicInteger();
    final AtomicInteger drops = new AtomicInteger();
    volatile CountDownLatch gate;

    Cdn(int size) {
      body = new byte[size];
      for (int i = 0; i < size; i++) body[i] = (byte) (i * 31 + 7);
    }

    @Override
    public DataSource createDataSource() {
      return new DataSource() {
        private int at, left;
        private Uri uri;
        private boolean open;

        @Override
        public long open(DataSpec spec) throws IOException {
          opens.add(spec.position + "-" + spec.length);
          synchronized (this) {
            open = true;
          }
          peak.accumulateAndGet(live.incrementAndGet(), Math::max);
          CountDownLatch wait = gate;
          if (wait != null) {
            try {
              wait.await(5, TimeUnit.SECONDS);
            } catch (InterruptedException e) {
              throw new java.io.InterruptedIOException();
            }
          }
          if (drops.getAndDecrement() > 0) throw new java.net.SocketTimeoutException("drop");
          uri = spec.uri;
          at = (int) spec.position;
          left = Math.max(0, body.length - at);
          if (spec.length != C.LENGTH_UNSET) left = Math.min(left, (int) spec.length);
          return left;
        }

        @Override
        public int read(byte[] buffer, int offset, int length) {
          if (left == 0) return C.RESULT_END_OF_INPUT;
          int n = Math.min(Math.min(length, left), 7); // short reads, like a socket
          System.arraycopy(body, at, buffer, offset, n);
          at += n;
          left -= n;
          return n;
        }

        @Override
        public Uri getUri() {
          return uri;
        }

        @Override
        public void addTransferListener(TransferListener listener) {}

        @Override
        public synchronized void close() {
          if (open) live.decrementAndGet();
          open = false;
        }
      };
    }
  }

  private ChunkedRangeDataSource source(Cdn cdn, int connections) {
    return new ChunkedRangeDataSource(
        cdn, IndexedRangeDataSource::youtubeCdn, 50, 100, connections, pool);
  }

  private static byte[] readAll(DataSource source, DataSpec spec) throws IOException {
    long length = source.open(spec);
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    byte[] buffer = new byte[64];
    int n;
    while ((n = source.read(buffer, 0, buffer.length)) != C.RESULT_END_OF_INPUT)
      out.write(buffer, 0, n);
    source.close();
    assertEquals(length, out.size());
    return out.toByteArray();
  }

  private static List<String> sorted(List<String> opens) {
    List<String> copy = new ArrayList<>(opens);
    copy.sort(Comparator.comparingLong(s -> Long.parseLong(s.substring(0, s.indexOf('-')))));
    return copy;
  }

  private static void await(AtomicInteger value, int expected) throws InterruptedException {
    long deadline = System.currentTimeMillis() + 5000;
    while (value.get() != expected && System.currentTimeMillis() < deadline) Thread.sleep(5);
    assertEquals(expected, value.get());
  }

  @Test
  public void anOpenEndedCdnReadIsFetchedInOrderOverParallelBoundedRequests() throws Exception {
    Cdn cdn = new Cdn(350);
    DataSpec spec =
        new DataSpec.Builder().setUri(Uri.parse(CDN + "&clen=350")).setPosition(20).build();

    // Every connection is in flight before the first byte is consumed, and close() aborts them.
    cdn.gate = new CountDownLatch(1);
    ChunkedRangeDataSource source = source(cdn, 3);
    assertEquals(330, source.open(spec));
    await(cdn.live, 3);
    source.close();
    cdn.gate.countDown();
    await(cdn.live, 0);

    cdn.opens.clear();
    cdn.peak.set(0);
    cdn.gate = null;
    assertArrayEquals(Arrays.copyOfRange(cdn.body, 20, 350), readAll(source(cdn, 3), spec));
    assertEquals(Arrays.asList("20-100", "120-100", "220-100", "320-30"), sorted(cdn.opens));
    assertTrue("at most 3 connections, saw " + cdn.peak.get(), cdn.peak.get() <= 3);
    await(cdn.live, 0);
  }

  @Test
  public void aSmallTrackIsStillBoundedAndSpreadAcrossConnections() throws IOException {
    Cdn cdn = new Cdn(120);
    DataSpec spec = new DataSpec.Builder().setUri(Uri.parse(CDN + "&clen=120")).build();
    assertArrayEquals(cdn.body, readAll(source(cdn, 4), spec));
    // ceil(120 / 4) = 30 is below the 50-byte minimum, so three bounded requests.
    assertEquals(Arrays.asList("0-50", "50-50", "100-20"), sorted(cdn.opens));
  }

  @Test
  public void aBoundedCdnReadKeepsTheOriginalUri() throws IOException {
    Cdn cdn = new Cdn(500);
    Uri uri = Uri.parse(CDN);
    DataSpec spec = new DataSpec.Builder().setUri(uri).setPosition(0).setLength(250).build();
    ChunkedRangeDataSource source = source(cdn, 2);
    source.open(spec);
    assertEquals(uri, source.getUri());
    source.close();
    cdn.opens.clear();
    assertArrayEquals(Arrays.copyOfRange(cdn.body, 0, 250), readAll(source(cdn, 2), spec));
    assertEquals(Arrays.asList("0-100", "100-100", "200-50"), sorted(cdn.opens));
  }

  @Test
  public void unknownLengthSignedPreRangedOrNonCdnReadsPassThroughUnchanged() throws IOException {
    for (String url :
        new String[] {
          CDN,
          CDN + "&clen=500&range=0-499",
          CDN + "&clen=500&sparams=range",
          "https://example.test/videoplayback?clen=500"
        }) {
      Cdn cdn = new Cdn(500);
      DataSpec spec = new DataSpec.Builder().setUri(Uri.parse(url)).build();
      readAll(source(cdn, 4), spec);
      assertEquals(url, Collections.singletonList("0--1"), cdn.opens);
    }
  }

  @Test
  public void aDroppedConnectionIsRetriedWithoutLosingBytes() throws IOException {
    Cdn cdn = new Cdn(300);
    cdn.drops.set(2);
    DataSpec spec = new DataSpec.Builder().setUri(Uri.parse(CDN + "&clen=300")).build();
    assertArrayEquals(cdn.body, readAll(source(cdn, 1), spec));
    assertEquals(5, cdn.opens.size()); // first chunk: two drops, then three good requests
  }

  @Test
  public void aShortChunkFailsInsteadOfCompletingATruncatedFile() throws IOException {
    Cdn cdn = new Cdn(150);
    DataSpec spec = new DataSpec.Builder().setUri(Uri.parse(CDN + "&clen=300")).build();
    ChunkedRangeDataSource source = source(cdn, 2);
    source.open(spec);
    byte[] buffer = new byte[64];
    assertThrows(
        java.io.EOFException.class,
        () -> {
          while (source.read(buffer, 0, buffer.length) != C.RESULT_END_OF_INPUT) {}
        });
    source.close();
  }

  @Test
  public void chunkSizeSpreadsOverConnectionsWithinBounds() {
    assertEquals(250, ChunkedRangeDataSource.chunkSize(1000, 4, 100, 400));
    assertEquals(100, ChunkedRangeDataSource.chunkSize(120, 4, 100, 400));
    assertEquals(400, ChunkedRangeDataSource.chunkSize(100_000, 4, 100, 400));
    assertEquals(
        2L * 1024 * 1024,
        Math.min(
            ChunkedRangeDataSource.MAX_CHUNK_BYTES,
            ChunkedRangeDataSource.BUFFER_BYTES / ChunkedRangeDataSource.CONNECTIONS));
  }
}
