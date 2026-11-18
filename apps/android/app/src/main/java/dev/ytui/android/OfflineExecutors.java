package dev.ytui.android;

import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** Media3 limits active downloads to at most four; each progressive transfer holds a worker. */
final class OfflineExecutors implements AutoCloseable {
  final ExecutorService metadata = Executors.newFixedThreadPool(3);
  final ExecutorService transfers = Executors.newFixedThreadPool(4);

  @Override public void close() {
    transfers.shutdownNow();
    metadata.shutdownNow();
  }
}
