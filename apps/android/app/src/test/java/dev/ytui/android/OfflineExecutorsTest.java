package dev.ytui.android;

import static org.junit.Assert.*;
import java.util.concurrent.*;
import org.junit.Test;

public class OfflineExecutorsTest {
  @Test public void metadataAndControlProgressWhileFourTransfersAreHeld() throws Exception {
    try (OfflineExecutors pools = new OfflineExecutors()) {
      CountDownLatch active = new CountDownLatch(4);
      CountDownLatch release = new CountDownLatch(1);
      java.util.List<Future<?>> transfers = new java.util.ArrayList<>();
      for (int i = 0; i < 4; i++) transfers.add(pools.transfers.submit(() -> {
        active.countDown();
        try { release.await(); } catch (InterruptedException cancelled) { Thread.currentThread().interrupt(); }
      }));
      assertTrue(active.await(2, TimeUnit.SECONDS));
      assertEquals("snapshot", pools.metadata.submit(() -> "snapshot").get(2, TimeUnit.SECONDS));
      pools.metadata.submit(() -> transfers.get(0).cancel(true)).get(2, TimeUnit.SECONDS);
      assertTrue(transfers.get(0).isCancelled());
      assertFalse(transfers.get(1).isDone());
      release.countDown();
      transfers.get(1).get(2, TimeUnit.SECONDS);
    }
  }

  @Test public void originalSharedPoolStarvesMetadataUntilTransferReleases() throws Exception {
    ExecutorService old = Executors.newFixedThreadPool(3);
    CountDownLatch active = new CountDownLatch(3), release = new CountDownLatch(1);
    try {
      for (int i = 0; i < 3; i++) old.submit(() -> {
        active.countDown();
        try { release.await(); } catch (InterruptedException e) { Thread.currentThread().interrupt(); }
      });
      assertTrue(active.await(2, TimeUnit.SECONDS));
      Future<String> snapshot = old.submit(() -> "snapshot");
      assertFalse(snapshot.isDone());
      release.countDown();
      assertEquals("snapshot", snapshot.get(2, TimeUnit.SECONDS));
    } finally { release.countDown(); old.shutdownNow(); }
  }
}
