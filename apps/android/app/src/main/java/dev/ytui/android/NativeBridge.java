package dev.ytui.android;

/** Shared Rust engine contract; Android presentation uses native Compose controls. */
public final class NativeBridge {
  static {
    System.loadLibrary("ytui_mobile");
  }

  private NativeBridge() {}

  public static native String init(String files, String cache);

  public static native String request(String json);

  public static native int[] thumbnail(String media, int pixels);

  public static native long beginAccountChange();

  public static native long accountIntent();

  public static native void cancel(long id);

  public static native void activateContext(String id);

  public static native void frameAvailable();

  public static native long uiGeneration();

  public static native long uiShutdowns();

  public static native long meter(java.nio.ByteBuffer buffer, int offset, int length, int channels);

  /**
   * Writes {@code out.length} log-spaced band levels (0..65535, low to high frequency) from the
   * same PCM window. The Rust side keeps a rolling mono window across calls.
   */
  public static native void spectrum(
      java.nio.ByteBuffer buffer, int offset, int length, int channels, int sampleRate, int[] out);
}
