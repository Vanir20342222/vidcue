package dev.ytui.android;

import android.net.Uri;
import androidx.media3.datasource.*;
import java.io.IOException;
import java.util.*;
import java.util.function.BooleanSupplier;

/** Sample-queue preparation is cache-only; promotion enables normal foreground reads. */
@androidx.annotation.OptIn(markerClass = androidx.media3.common.util.UnstableApi.class)
final class PreparedPlaybackDataSource implements DataSource {
  private final DataSource.Factory cached, foreground;
  private final BooleanSupplier promoted;
  private final List<TransferListener> listeners = new ArrayList<>();
  private DataSource delegate;
  private DataSpec opened;
  private long consumed;
  private boolean usingForeground;

  PreparedPlaybackDataSource(
      DataSource.Factory cached, DataSource.Factory foreground, BooleanSupplier promoted) {
    this.cached = cached;
    this.foreground = foreground;
    this.promoted = promoted;
  }

  @Override public void addTransferListener(TransferListener listener) { listeners.add(listener); }

  @Override public long open(DataSpec spec) throws IOException {
    opened = spec;
    consumed = 0;
    usingForeground = promoted.getAsBoolean();
    return openDelegate(spec);
  }

  private long openDelegate(DataSpec spec) throws IOException {
    delegate = (usingForeground ? foreground : cached).createDataSource();
    for (TransferListener listener : listeners) delegate.addTransferListener(listener);
    return delegate.open(spec);
  }

  @Override public int read(byte[] buffer, int offset, int length) throws IOException {
    if (length == 0) return 0;
    if (!usingForeground && promoted.getAsBoolean()) {
      // A suspended loader may retain its cache-only source. Upgrade that same
      // read at its current byte offset, avoiding a cache-miss/retry stall.
      if (opened.length != androidx.media3.common.C.LENGTH_UNSET && consumed >= opened.length)
        return androidx.media3.common.C.RESULT_END_OF_INPUT;
      delegate.close();
      usingForeground = true;
      DataSpec.Builder next = opened.buildUpon().setPosition(opened.position + consumed);
      if (opened.length != androidx.media3.common.C.LENGTH_UNSET)
        next.setLength(opened.length - consumed);
      openDelegate(next.build());
    }
    int read = delegate.read(buffer, offset, length);
    if (read > 0) consumed += read;
    return read;
  }

  @Override public Uri getUri() { return delegate == null ? null : delegate.getUri(); }

  @Override public Map<String, List<String>> getResponseHeaders() {
    return delegate == null ? Collections.emptyMap() : delegate.getResponseHeaders();
  }

  @Override public void close() throws IOException {
    if (delegate != null) {
      try { delegate.close(); } finally { delegate = null; }
    }
  }
}
