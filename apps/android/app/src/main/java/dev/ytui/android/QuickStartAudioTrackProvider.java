package dev.ytui.android;

import android.content.Context;
import android.media.AudioTrack;
import androidx.media3.common.AudioAttributes;
import androidx.media3.common.util.UnstableApi;
import androidx.media3.exoplayer.audio.AudioSink;
import androidx.media3.exoplayer.audio.DefaultAudioSink;
import androidx.media3.exoplayer.audio.DefaultAudioTrackProvider;

/**
 * Lets a new AudioTrack start playing as soon as its first audio is written. Since Android 12 a
 * streaming track waits until its whole buffer (about 250 ms in Media3) is filled before the
 * output starts; a low start threshold removes that wait from every song start without making
 * the buffer itself smaller.
 */
@UnstableApi
final class QuickStartAudioTrackProvider implements DefaultAudioSink.AudioTrackProvider {
  /** About 40 ms of audio at the track's own rate. */
  private static final int START_MS = 40;

  private final DefaultAudioTrackProvider delegate = new DefaultAudioTrackProvider();

  @Override
  public AudioTrack getAudioTrack(
      AudioSink.AudioTrackConfig config, AudioAttributes attributes, int sessionId, Context context) {
    AudioTrack track = delegate.getAudioTrack(config, attributes, sessionId, context);
    if (android.os.Build.VERSION.SDK_INT >= 31 && !config.offload && !config.tunneling) {
      try {
        int frames = Math.max(1, config.sampleRate * START_MS / 1000);
        track.setStartThresholdInFrames(Math.min(frames, track.getBufferSizeInFrames()));
      } catch (RuntimeException unsupported) {
        // Keep the platform default when a device refuses a lower threshold.
      }
    }
    return track;
  }

  @Override
  public int getAudioTrackChannelConfig(int channelCount) {
    return delegate.getAudioTrackChannelConfig(channelCount);
  }
}
