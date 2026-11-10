package dev.ytui.android;

import android.media.MediaCodec;
import android.media.MediaDataSource;
import android.media.MediaExtractor;
import android.media.MediaFormat;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.function.BooleanSupplier;

/**
 * Decodes the first audio track of a media source to 16-bit PCM with the platform codecs (AAC,
 * Opus, Vorbis, ...). The source is read through [MediaDataSource], so cached offline files are
 * decoded without copying them first.
 */
final class Mp3Decoder {
  /** Receives decoded audio. [format] is called first and again if the codec changes it. */
  interface Sink {
    void format(int sampleRate, int channels) throws IOException;

    /** Interleaved signed 16-bit samples; [frames] frames of [channels] shorts each. */
    void pcm(short[] data, int frames) throws IOException;
  }

  private Mp3Decoder() {}

  /** Decodes to [sink]; returns the number of sample frames delivered. */
  static long decode(MediaDataSource source, Sink sink, BooleanSupplier cancelled)
      throws IOException {
    MediaExtractor extractor = new MediaExtractor();
    MediaCodec codec = null;
    long total = 0;
    try {
      extractor.setDataSource(source);
      int track = -1;
      MediaFormat input = null;
      for (int i = 0; i < extractor.getTrackCount() && track < 0; i++) {
        MediaFormat format = extractor.getTrackFormat(i);
        String mime = format.getString(MediaFormat.KEY_MIME);
        if (mime != null && mime.startsWith("audio/")) {
          track = i;
          input = format;
        }
      }
      if (track < 0) throw new IOException("No audio track");
      extractor.selectTrack(track);
      codec = MediaCodec.createDecoderByType(input.getString(MediaFormat.KEY_MIME));
      codec.configure(input, null, null, 0);
      codec.start();
      MediaCodec.BufferInfo info = new MediaCodec.BufferInfo();
      int rate = input.getInteger(MediaFormat.KEY_SAMPLE_RATE);
      int channels = input.getInteger(MediaFormat.KEY_CHANNEL_COUNT);
      boolean announced = false, inputDone = false, outputDone = false;
      boolean floatPcm = false;
      while (!outputDone) {
        if (cancelled.getAsBoolean() || Thread.currentThread().isInterrupted())
          throw new java.io.InterruptedIOException();
        if (!inputDone) {
          int index = codec.dequeueInputBuffer(10_000);
          if (index >= 0) {
            ByteBuffer buffer = codec.getInputBuffer(index);
            int size = extractor.readSampleData(buffer, 0);
            if (size < 0) {
              codec.queueInputBuffer(index, 0, 0, 0, MediaCodec.BUFFER_FLAG_END_OF_STREAM);
              inputDone = true;
            } else {
              codec.queueInputBuffer(index, 0, size, extractor.getSampleTime(), 0);
              extractor.advance();
            }
          }
        }
        int index = codec.dequeueOutputBuffer(info, 10_000);
        if (index == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED) {
          MediaFormat output = codec.getOutputFormat();
          rate = output.getInteger(MediaFormat.KEY_SAMPLE_RATE);
          channels = output.getInteger(MediaFormat.KEY_CHANNEL_COUNT);
          floatPcm =
              output.containsKey(MediaFormat.KEY_PCM_ENCODING)
                  && output.getInteger(MediaFormat.KEY_PCM_ENCODING)
                      == android.media.AudioFormat.ENCODING_PCM_FLOAT;
          announced = false;
        } else if (index >= 0) {
          if (info.size > 0) {
            if (!announced) {
              sink.format(rate, Math.min(channels, 2));
              announced = true;
            }
            ByteBuffer out = codec.getOutputBuffer(index);
            out.position(info.offset).limit(info.offset + info.size);
            out.order(ByteOrder.nativeOrder());
            int frames = info.size / ((floatPcm ? 4 : 2) * channels);
            short[] pcm = downmix(out, frames, channels, floatPcm);
            sink.pcm(pcm, frames);
            total += frames;
          }
          codec.releaseOutputBuffer(index, false);
          outputDone = (info.flags & MediaCodec.BUFFER_FLAG_END_OF_STREAM) != 0;
        }
      }
      return total;
    } finally {
      if (codec != null)
        try {
          codec.stop();
        } catch (RuntimeException ignored) {
        }
      if (codec != null) codec.release();
      extractor.release();
    }
  }

  /** Mono stays mono; anything with more than one channel becomes stereo from its first two. */
  static short[] downmix(ByteBuffer in, int frames, int channels, boolean floatPcm) {
    int outChannels = Math.min(channels, 2);
    short[] out = new short[frames * outChannels];
    for (int f = 0; f < frames; f++) {
      for (int c = 0; c < channels; c++) {
        short sample;
        if (floatPcm) {
          float value = in.getFloat();
          sample = (short) Math.max(-32768, Math.min(32767, Math.round(value * 32768f)));
        } else sample = in.getShort();
        if (c < outChannels) out[f * outChannels + c] = sample;
      }
    }
    return out;
  }
}
