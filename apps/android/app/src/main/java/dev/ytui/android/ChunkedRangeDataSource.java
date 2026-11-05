package dev.ytui.android;

import android.net.Uri;
import androidx.media3.common.C;
import androidx.media3.datasource.*;
import java.io.IOException;
import java.util.*;
import java.util.function.Predicate;

/**
 * Splits one long YouTube CDN read into consecutive bounded requests.
 *
 * <p>googlevideo.com paces an open-ended or very large request at roughly the media bitrate, so
 * an offline download through Media3's single progressive request advanced at playback speed
 * (tens of KB/s for audio). Bounded requests of at most {@link #CHUNK_BYTES} are served at
 * link speed, which is the same approach yt-dlp uses for YouTube ({@code http_chunk_size}).
 * Cache positions and the reported URI stay those of the original request.
 */
@androidx.annotation.OptIn(markerClass = androidx.media3.common.util.UnstableApi.class)
final class ChunkedRangeDataSource implements DataSource {
  /** yt-dlp's YouTube chunk size; larger bounded requests are throttled again. */
  static final long CHUNK_BYTES = 10L * 1024 * 1024;

  private final DataSource upstream;
  private final Predicate<Uri> host;
  private final long chunkBytes;
  private DataSpec spec;
  private boolean chunked;
  private boolean open;
  private long next;
  private long end;
  private long chunkRemaining;

  ChunkedRangeDataSource(DataSource upstream, Predicate<Uri> host, long chunkBytes) {
    if (chunkBytes <= 0) throw new IllegalArgumentException("chunkBytes");
    this.upstream = upstream;
    this.host = host;
    this.chunkBytes = chunkBytes;
  }

  /** Download source: bounded chunks over the query-range CDN source used by playback. */
  static DataSource.Factory factory(HttpDataSource.Factory http) {
    DataSource.Factory ranged = IndexedRangeDataSource.factory(http);
    return () ->
        new ChunkedRangeDataSource(
            ranged.createDataSource(), IndexedRangeDataSource::youtubeCdn, CHUNK_BYTES);
  }

  /** Exclusive end of the requested bytes, or {@link C#LENGTH_UNSET} when it cannot be known. */
  static long end(DataSpec spec) {
    if (spec.length != C.LENGTH_UNSET) {
      return spec.position > Long.MAX_VALUE - spec.length
          ? C.LENGTH_UNSET
          : spec.position + spec.length;
    }
    String clen = spec.uri.getQueryParameter("clen");
    if (clen == null) return C.LENGTH_UNSET;
    try {
      long size = Long.parseLong(clen);
      return size > spec.position ? size : C.LENGTH_UNSET;
    } catch (NumberFormatException ignored) {
      return C.LENGTH_UNSET;
    }
  }

  private static boolean signedRange(Uri uri) {
    for (String key : new String[] {"sparams", "lsparams"}) {
      String signed = uri.getQueryParameter(key);
      if (signed != null && Arrays.asList(signed.split(",")).contains("range")) return true;
    }
    return false;
  }

  @Override
  public long open(DataSpec spec) throws IOException {
    this.spec = spec;
    chunked = false;
    long end =
        host.test(spec.uri)
                && spec.httpMethod == DataSpec.HTTP_METHOD_GET
                && spec.httpBody == null
                && spec.httpRequestHeaders.keySet().stream()
                    .noneMatch(k -> k.equalsIgnoreCase("Range"))
                && spec.uri.getQueryParameter("range") == null
                && !signedRange(spec.uri)
            ? end(spec)
            : C.LENGTH_UNSET;
    if (end == C.LENGTH_UNSET || end - spec.position <= chunkBytes) {
      open = true;
      return upstream.open(spec);
    }
    this.end = end;
    next = spec.position;
    chunked = true;
    openChunk();
    return end - spec.position;
  }

  private void openChunk() throws IOException {
    long length = Math.min(chunkBytes, end - next);
    DataSpec chunk = spec.buildUpon().setPosition(next).setLength(length).build();
    open = true;
    long opened = upstream.open(chunk);
    if (opened != C.LENGTH_UNSET && opened < length)
      throw new java.io.EOFException("Media range ended before its declared length");
    next += length;
    chunkRemaining = length;
  }

  @Override
  public int read(byte[] buffer, int offset, int length) throws IOException {
    if (length == 0) return 0;
    if (!chunked) return upstream.read(buffer, offset, length);
    while (chunkRemaining == 0) {
      if (next >= end) return C.RESULT_END_OF_INPUT;
      open = false;
      upstream.close();
      openChunk();
    }
    int read = upstream.read(buffer, offset, (int) Math.min(length, chunkRemaining));
    if (read == C.RESULT_END_OF_INPUT)
      throw new java.io.EOFException("Media chunk ended before its declared length");
    chunkRemaining -= read;
    return read;
  }

  @Override
  public Uri getUri() {
    return chunked && spec != null ? spec.uri : upstream.getUri();
  }

  @Override
  public Map<String, List<String>> getResponseHeaders() {
    return upstream.getResponseHeaders();
  }

  @Override
  public void addTransferListener(TransferListener listener) {
    upstream.addTransferListener(listener);
  }

  @Override
  public void close() throws IOException {
    try {
      if (open) upstream.close();
    } finally {
      open = false;
      chunked = false;
      spec = null;
    }
  }
}
