package dev.ytui.android;

import static org.junit.Assert.*;

import android.net.Uri;
import androidx.media3.common.C;
import androidx.media3.datasource.*;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.util.*;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;

@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 35)
@androidx.annotation.OptIn(markerClass = androidx.media3.common.util.UnstableApi.class)
public final class ChunkedRangeDataSourceTest {
  private static final String CDN = "https://r1.googlevideo.com/videoplayback?id=1";

  /** Serves exactly the requested slice and records each request as "position-length". */
  private static final class Fake implements DataSource {
    final byte[] body;
    final List<String> opens = new ArrayList<>();
    int closes;
    private int at, left;
    private Uri uri;

    Fake(int size) {
      body = new byte[size];
      for (int i = 0; i < size; i++) body[i] = (byte) (i * 31 + 7);
    }

    @Override
    public long open(DataSpec spec) {
      opens.add(spec.position + "-" + spec.length);
      uri = spec.uri;
      at = (int) spec.position;
      left = body.length - at;
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
    public void close() {
      closes++;
    }
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

  private static ChunkedRangeDataSource chunked(Fake fake) {
    return new ChunkedRangeDataSource(fake, IndexedRangeDataSource::youtubeCdn, 100);
  }

  @Test
  public void anOpenEndedCdnReadUsesBoundedConsecutiveRequestsFromClen() throws IOException {
    Fake fake = new Fake(350);
    DataSpec spec =
        new DataSpec.Builder().setUri(Uri.parse(CDN + "&clen=350")).setPosition(20).build();
    assertArrayEquals(Arrays.copyOfRange(fake.body, 20, 350), readAll(chunked(fake), spec));
    assertEquals(Arrays.asList("20-100", "120-100", "220-100", "320-30"), fake.opens);
    assertEquals(fake.opens.size(), fake.closes);
  }

  @Test
  public void aBoundedCdnReadIsSplitAndKeepsTheOriginalUri() throws IOException {
    Fake fake = new Fake(500);
    Uri uri = Uri.parse(CDN);
    DataSpec spec = new DataSpec.Builder().setUri(uri).setPosition(0).setLength(250).build();
    ChunkedRangeDataSource source = chunked(fake);
    source.open(spec);
    assertEquals(uri, source.getUri());
    source.close();
    fake.opens.clear();
    assertArrayEquals(Arrays.copyOfRange(fake.body, 0, 250), readAll(source, spec));
    assertEquals(Arrays.asList("0-100", "100-100", "200-50"), fake.opens);
  }

  @Test
  public void smallUnknownOrNonCdnReadsPassThroughUnchanged() throws IOException {
    for (String url :
        new String[] {
          CDN + "&clen=80",
          CDN,
          CDN + "&clen=500&range=0-499",
          CDN + "&clen=500&sparams=range",
          "https://example.test/videoplayback?clen=500"
        }) {
      Fake fake = new Fake(500);
      DataSpec spec = new DataSpec.Builder().setUri(Uri.parse(url)).build();
      readAll(chunked(fake), spec);
      assertEquals(url, Collections.singletonList("0--1"), fake.opens);
    }
  }

  @Test
  public void aShortChunkFailsInsteadOfCompletingATruncatedFile() throws IOException {
    Fake fake = new Fake(150);
    DataSpec spec = new DataSpec.Builder().setUri(Uri.parse(CDN + "&clen=300")).build();
    ChunkedRangeDataSource source = chunked(fake);
    source.open(spec);
    byte[] buffer = new byte[64];
    assertThrows(
        java.io.EOFException.class,
        () -> {
          while (source.read(buffer, 0, buffer.length) != C.RESULT_END_OF_INPUT) {}
        });
    source.close();
  }
}
