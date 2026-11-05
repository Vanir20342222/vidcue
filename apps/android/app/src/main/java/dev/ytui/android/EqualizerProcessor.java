package dev.ytui.android;

import androidx.media3.common.C;
import androidx.media3.common.audio.BaseAudioProcessor;
import java.nio.ByteBuffer;

/**
 * Applies the saved ten-band equalizer (Settings → Playback) to decoded 16-bit PCM. The curve
 * and filter maths live in the Rust engine, which follows saved settings directly; "off" copies
 * the audio through untouched. It runs before {@link AudioTap} so visualizers show what plays.
 */
@androidx.annotation.OptIn(markerClass = androidx.media3.common.util.UnstableApi.class)
public final class EqualizerProcessor extends BaseAudioProcessor {
  @Override
  public AudioFormat onConfigure(AudioFormat input) {
    return input.encoding == C.ENCODING_PCM_16BIT ? input : AudioFormat.NOT_SET;
  }

  @Override
  public void queueInput(ByteBuffer input) {
    int size = input.remaining();
    if (size == 0) return; // EOS may pass the shared EMPTY_BUFFER; never put a buffer into itself.
    ByteBuffer output = replaceOutputBuffer(size);
    output.put(input);
    output.flip();
    if (NativeBridge.equalizerEnabled())
      NativeBridge.equalize(
          output, 0, size, inputAudioFormat.channelCount, inputAudioFormat.sampleRate);
  }

  @Override
  protected void onFlush() {
    // A seek or new stream must not ring with the previous position's filter memory.
    NativeBridge.equalizerReset();
  }
}
