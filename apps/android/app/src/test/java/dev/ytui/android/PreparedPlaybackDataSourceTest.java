package dev.ytui.android;

import static org.junit.Assert.*;
import androidx.media3.datasource.*;
import java.io.IOException;
import java.util.concurrent.atomic.*;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;

@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 35)
public final class PreparedPlaybackDataSourceTest {
  @Test public void speculativeReadsCannotReachNetworkButPromotionCanContinue() throws Exception {
    AtomicBoolean promoted = new AtomicBoolean();
    AtomicInteger network = new AtomicInteger();
    PreparedPlaybackDataSource source = new PreparedPlaybackDataSource(
        () -> new ByteArrayDataSource(new byte[] {1, 2}),
        () -> { network.incrementAndGet(); return new ByteArrayDataSource(new byte[] {1, 2, 3, 4}); },
        promoted::get);
    DataSpec first = new DataSpec.Builder().setUri("https://example.invalid/item").build();
    assertEquals(2, source.open(first));
    source.close();
    assertEquals(0, network.get());
    promoted.set(true);
    assertEquals(2, source.open(first.buildUpon().setPosition(2).build()));
    byte[] bytes = new byte[2];
    assertEquals(2, source.read(bytes, 0, 2));
    assertArrayEquals(new byte[] {3, 4}, bytes);
    source.close();
    assertEquals(1, network.get());
  }

  @Test public void promotionUpgradesAnAlreadyOpenLoaderWithoutRepeatingBytes() throws Exception {
    AtomicBoolean promoted = new AtomicBoolean();
    AtomicInteger network = new AtomicInteger();
    PreparedPlaybackDataSource source = new PreparedPlaybackDataSource(
        () -> new ByteArrayDataSource(new byte[] {1, 2, 3, 4}),
        () -> { network.incrementAndGet(); return new ByteArrayDataSource(new byte[] {1, 2, 3, 4}); },
        promoted::get);
    source.open(new DataSpec.Builder().setUri("https://example.invalid/item").setLength(4).build());
    byte[] bytes = new byte[2];
    assertEquals(2, source.read(bytes, 0, 2));
    assertArrayEquals(new byte[] {1, 2}, bytes);
    assertEquals(0, network.get());
    promoted.set(true);
    assertEquals(2, source.read(bytes, 0, 2));
    assertArrayEquals(new byte[] {3, 4}, bytes);
    assertEquals(1, network.get());
    source.close();
  }

  @Test public void cacheMissDoesNotFetchUntilForegroundRetry() throws Exception {
    AtomicBoolean promoted = new AtomicBoolean();
    AtomicInteger network = new AtomicInteger();
    PreparedPlaybackDataSource source = new PreparedPlaybackDataSource(
        () -> new ByteArrayDataSource(new byte[] {1}),
        () -> { network.incrementAndGet(); return new ByteArrayDataSource(new byte[] {1, 2, 3}); },
        promoted::get);
    DataSpec spec = new DataSpec.Builder().setUri("https://example.invalid/item").setPosition(2).build();
    assertThrows(IOException.class, () -> source.open(spec));
    source.close();
    assertEquals(0, network.get());
    promoted.set(true);
    assertEquals(1, source.open(spec));
    source.close();
    assertEquals(1, network.get());
  }
}
