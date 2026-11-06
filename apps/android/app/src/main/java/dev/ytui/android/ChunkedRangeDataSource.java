package dev.ytui.android;

import android.net.Uri;
import androidx.media3.common.C;
import androidx.media3.datasource.*;
import java.io.IOException;
import java.io.InterruptedIOException;
import java.util.*;
import java.util.concurrent.*;
import java.util.function.Predicate;

/**
 * Reads one YouTube CDN resource as bounded chunks fetched over parallel connections.
 *
 * <p>googlevideo.com paces an open-ended or very large request at roughly the media bitrate, so
 * Media3's single progressive request downloaded at playback speed (tens of KB/s for audio).
 * Bounded requests of at most {@link #MAX_CHUNK_BYTES} (yt-dlp's {@code http_chunk_size}) are
 * served at link speed, and several in flight hide per-request latency and per-connection
 * limits. Chunks are buffered and returned strictly in order, so the cache sees one ordinary
 * sequential read with the original URI, positions and cache key.
 */
@androidx.annotation.OptIn(markerClass = androidx.media3.common.util.UnstableApi.class)
final class ChunkedRangeDataSource implements DataSource {
  /** Larger bounded requests are throttled again. */
  static final long MAX_CHUNK_BYTES = 10L * 1024 * 1024;

  /** Small enough that a short track still spreads over several connections. */
  static final long MIN_CHUNK_BYTES = 512L * 1024;

  /** Connections per resource; also bounds buffered memory to this many chunks. */
  static final int CONNECTIONS = 4;

  /** Buffered bytes per resource: CONNECTIONS chunks of at most BUFFER_BYTES / CONNECTIONS. */
  static final long BUFFER_BYTES = 8L * 1024 * 1024;

  private static final ExecutorService POOL =
      Executors.newCachedThreadPool(
          task -> {
            Thread thread = new Thread(task, "vidcue-download-chunk");
            thread.setDaemon(true);
            return thread;
          });

  private final DataSource.Factory upstream;
  private final Predicate<Uri> host;
  private final long minChunk, maxChunk;
  private final int connections;
  private final ExecutorService pool;
  private final List<TransferListener> listeners = new ArrayList<>();
  private final ArrayDeque<Future<byte[]>> pending = new ArrayDeque<>();
  private final Set<DataSource> active = ConcurrentHashMap.newKeySet();

  private DataSource direct;
  private DataSpec spec;
  private boolean chunked;
  private long next, end, chunk;
  private volatile boolean closed;
  private byte[] current;
  private int currentAt;

  ChunkedRangeDataSource(
      DataSource.Factory upstream,
      Predicate<Uri> host,
      long minChunk,
      long maxChunk,
      int connections,
      ExecutorService pool) {
    if (minChunk <= 0 || maxChunk < minChunk || maxChunk > Integer.MAX_VALUE || connections < 1)
      throw new IllegalArgumentException("chunking");
    this.upstream = upstream;
    this.host = host;
    this.minChunk = minChunk;
    this.maxChunk = maxChunk;
    this.connections = connections;
    this.pool = pool;
  }

  /** Download source: parallel bounded chunks over playback's query-range CDN source. */
  static DataSource.Factory factory(HttpDataSource.Factory http) {
    DataSource.Factory ranged = IndexedRangeDataSource.factory(http);
    return () ->
        new ChunkedRangeDataSource(
            ranged,
            IndexedRangeDataSource::youtubeCdn,
            MIN_CHUNK_BYTES,
            Math.min(MAX_CHUNK_BYTES, BUFFER_BYTES / CONNECTIONS),
            CONNECTIONS,
            POOL);
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

  /** Chunk size: spread over every connection, within [min, max]. */
  static long chunkSize(long total, int connections, long min, long max) {
    long even = (total + connections - 1) / connections;
    return Math.max(min, Math.min(max, even));
  }

  private static boolean signedRange(Uri uri) {
    for (String key : new String[] {"sparams", "lsparams"}) {
      String signed = uri.getQueryParameter(key);
      if (signed != null && Arrays.asList(signed.split(",")).contains("range")) return true;
    }
    return false;
  }

  private boolean eligible(DataSpec spec) {
    return host.test(spec.uri)
        && spec.httpMethod == DataSpec.HTTP_METHOD_GET
        && spec.httpBody == null
        && spec.httpRequestHeaders.keySet().stream().noneMatch(k -> k.equalsIgnoreCase("Range"))
        && spec.uri.getQueryParameter("range") == null
        && !signedRange(spec.uri);
  }

  private DataSource create() {
    DataSource source = upstream.createDataSource();
    synchronized (listeners) {
      for (TransferListener listener : listeners) source.addTransferListener(listener);
    }
    return source;
  }

  @Override
  public long open(DataSpec spec) throws IOException {
    this.spec = spec;
    closed = false;
    long end = eligible(spec) ? end(spec) : C.LENGTH_UNSET;
    if (end == C.LENGTH_UNSET) {
      chunked = false;
      direct = create();
      return direct.open(spec);
    }
    chunked = true;
    this.end = end;
    next = spec.position;
    chunk = chunkSize(end - spec.position, connections, minChunk, maxChunk);
    current = null;
    currentAt = 0;
    while (pending.size() < connections && next < end) submit();
    return end - spec.position;
  }

  private void submit() {
    long position = next;
    int length = (int) Math.min(chunk, end - next);
    next += length;
    DataSpec part = spec.buildUpon().setPosition(position).setLength(length).build();
    pending.add(pool.submit(() -> fetch(part, length)));
  }

  /** Extra attempts per chunk after a dropped connection; HTTP status errors are not retried. */
  static final int CHUNK_RETRIES = 2;

  private byte[] fetch(DataSpec part, int length) throws IOException {
    byte[] bytes = new byte[length];
    for (int attempt = 0; ; attempt++) {
      try {
        return fetchOnce(part, bytes);
      } catch (HttpDataSource.InvalidResponseCodeException e) {
        throw e; // 403/429 etc. are not transient here; the download reports them
      } catch (IOException e) {
        // Timeouts and resets are retried; a close() during the attempt is not.
        if (closed || attempt >= CHUNK_RETRIES) throw e;
      }
    }
  }

  private byte[] fetchOnce(DataSpec part, byte[] bytes) throws IOException {
    int length = bytes.length;
    DataSource source = create();
    active.add(source);
    try {
      if (closed) throw new InterruptedIOException("Download cancelled");
      long opened = source.open(part);
      if (opened != C.LENGTH_UNSET && opened < length)
        throw new java.io.EOFException("Media range ended before its declared length");
      int at = 0;
      while (at < length) {
        if (closed) throw new InterruptedIOException("Download cancelled");
        int read = source.read(bytes, at, length - at);
        if (read == C.RESULT_END_OF_INPUT)
          throw new java.io.EOFException("Media chunk ended before its declared length");
        at += read;
      }
      return bytes;
    } finally {
      active.remove(source);
      source.close();
    }
  }

  private byte[] take() throws IOException {
    Future<byte[]> head = pending.poll();
    try {
      byte[] bytes = head.get();
      // Its connection is free: start the next chunk while this one is consumed.
      if (next < end) submit();
      return bytes;
    } catch (InterruptedException e) {
      Thread.currentThread().interrupt();
      throw new InterruptedIOException("Download interrupted");
    } catch (ExecutionException e) {
      Throwable cause = e.getCause();
      if (cause instanceof IOException) throw (IOException) cause;
      if (cause instanceof RuntimeException) throw (RuntimeException) cause;
      throw new IOException(cause);
    }
  }

  @Override
  public int read(byte[] buffer, int offset, int length) throws IOException {
    if (length == 0) return 0;
    if (!chunked) return direct.read(buffer, offset, length);
    if (current == null || currentAt == current.length) {
      current = null;
      if (pending.isEmpty()) return C.RESULT_END_OF_INPUT;
      current = take();
      currentAt = 0;
    }
    int read = Math.min(length, current.length - currentAt);
    System.arraycopy(current, currentAt, buffer, offset, read);
    currentAt += read;
    return read;
  }

  @Override
  public Uri getUri() {
    if (chunked) return spec == null ? null : spec.uri;
    return direct == null ? null : direct.getUri();
  }

  @Override
  public Map<String, List<String>> getResponseHeaders() {
    return chunked || direct == null ? Collections.emptyMap() : direct.getResponseHeaders();
  }

  @Override
  public void addTransferListener(TransferListener listener) {
    synchronized (listeners) {
      listeners.add(listener);
    }
  }

  @Override
  public void close() throws IOException {
    closed = true;
    for (Future<byte[]> future : pending) future.cancel(true);
    pending.clear();
    // Abort in-flight sockets so a pause or removal releases bandwidth at once.
    for (DataSource source : active) {
      try {
        source.close();
      } catch (IOException | RuntimeException ignored) {
      }
    }
    current = null;
    chunked = false;
    spec = null;
    DataSource opened = direct;
    direct = null;
    if (opened != null) opened.close();
  }
}
