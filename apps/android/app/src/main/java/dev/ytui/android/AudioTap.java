package dev.ytui.android;

import androidx.media3.common.C;
import androidx.media3.common.audio.BaseAudioProcessor;
import java.nio.ByteBuffer;

/**
 * A pass-through tap; numerical analysis is Rust and never records audio. {@code spectrum} is
 * written in place from the audio thread and read by the frame loop. A torn read only shows as
 * one slightly stale visual frame, so it is intentionally not synchronized.
 */
@androidx.annotation.OptIn(markerClass = androidx.media3.common.util.UnstableApi.class)
public final class AudioTap extends BaseAudioProcessor {
  static volatile long levels;
  static volatile boolean enabled;
  static final int[] spectrum = new int[24];

  @Override
  public AudioFormat onConfigure(AudioFormat input) {
    return input.encoding == C.ENCODING_PCM_16BIT ? input : AudioFormat.NOT_SET;
  }

  @Override
  public void queueInput(ByteBuffer input) {
    int size = input.remaining();
    if (size == 0) return; // EOS may pass the shared EMPTY_BUFFER; never put a buffer into itself.
    if (enabled && size > 0) {
      levels = NativeBridge.meter(input, input.position(), size, inputAudioFormat.channelCount);
      NativeBridge.spectrum(
          input,
          input.position(),
          size,
          inputAudioFormat.channelCount,
          inputAudioFormat.sampleRate,
          spectrum);
    }
    ByteBuffer output = replaceOutputBuffer(size);
    output.put(input);
    output.flip();
  }
}
