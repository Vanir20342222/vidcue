package dev.ytui.android;

import de.sciss.jump3r.mp3.BRHist;
import de.sciss.jump3r.mp3.BitStream;
import de.sciss.jump3r.mp3.GainAnalysis;
import de.sciss.jump3r.mp3.ID3Tag;
import de.sciss.jump3r.mp3.Lame;
import de.sciss.jump3r.mp3.LameGlobalFlags;
import de.sciss.jump3r.mp3.Presets;
import de.sciss.jump3r.mp3.Quantize;
import de.sciss.jump3r.mp3.QuantizePVT;
import de.sciss.jump3r.mp3.Reservoir;
import de.sciss.jump3r.mp3.Takehiro;
import de.sciss.jump3r.mp3.VBRTag;
import de.sciss.jump3r.mp3.Version;
import de.sciss.jump3r.mpg.Common;
import de.sciss.jump3r.mpg.Interface;
import de.sciss.jump3r.mpg.MPGLib;
import java.io.IOException;
import java.io.OutputStream;

/**
 * MP3 encoder over the pure-Java LAME port (jump3r, LGPL-2.1). It drives the core classes
 * directly because jump3r's own {@code lowlevel.LameEncoder} depends on javax.sound, which does
 * not exist on Android. Plain JVM code, so it is unit tested without a device.
 *
 * <p>Constant bitrate on purpose: a CBR stream needs no Xing header rewrite, so it can be written
 * straight to a stream that cannot seek (a document provider output).
 */
final class Mp3Encoder implements AutoCloseable {
  /** Default bitrate in kbit/s. 192 keeps most of a 128-256 kbit/s AAC or Opus source. */
  static final int BITRATE_KBPS = 192;

  private final Lame lame;
  private final LameGlobalFlags flags;
  private final int channels;
  private final OutputStream output;
  private final byte[] encoded;
  private boolean finished;

  /**
   * @param sampleRate input rate in Hz; LAME resamples to a rate MP3 supports when needed
   * @param channels 1 (mono) or 2 (stereo)
   */
  Mp3Encoder(int sampleRate, int channels, int bitrateKbps, OutputStream output) {
    if (channels < 1 || channels > 2)
      throw new IllegalArgumentException("MP3 supports one or two channels");
    if (sampleRate < 8000 || sampleRate > 192000)
      throw new IllegalArgumentException("Unsupported sample rate");
    this.channels = channels;
    this.output = output;
    lame = new Lame();
    GainAnalysis ga = new GainAnalysis();
    BitStream bs = new BitStream();
    Presets presets = new Presets();
    QuantizePVT qupvt = new QuantizePVT();
    Quantize quantize = new Quantize();
    VBRTag vbr = new VBRTag();
    Version version = new Version();
    ID3Tag id3 = new ID3Tag();
    Reservoir reservoir = new Reservoir();
    Takehiro takehiro = new Takehiro();
    BRHist hist = new BRHist();
    MPGLib mpg = new MPGLib();
    Interface intf = new Interface();
    Common common = new Common();
    lame.setModules(ga, bs, presets, qupvt, quantize, vbr, version, id3, mpg);
    bs.setModules(ga, mpg, version, vbr);
    id3.setModules(bs, version);
    presets.setModules(lame);
    quantize.setModules(bs, reservoir, qupvt, takehiro);
    qupvt.setModules(takehiro, reservoir, lame.enc.psy);
    reservoir.setModules(bs);
    takehiro.setModules(qupvt);
    vbr.setModules(lame, bs, version);
    mpg.setModules(intf, common);
    intf.setModules(vbr, common);
    flags = lame.lame_init();
    flags.num_channels = channels;
    flags.in_samplerate = sampleRate;
    flags.brate = bitrateKbps;
    flags.quality = 2; // LAME's "near-best" quality setting
    id3.id3tag_init(flags);
    // No ID3 header: the file starts at the first MPEG frame, which keeps it easy to verify.
    flags.write_id3tag_automatic = false;
    if (lame.lame_init_params(flags) < 0) throw new IllegalArgumentException("LAME rejected the settings");
    // Worst case from the LAME manual: 1.25 * samples + 7200 bytes, for one block.
    encoded = new byte[(int) (1.25 * BLOCK_FRAMES) + 7200 + 16384];
  }

  /** Frames encoded per call; larger blocks only need a larger output buffer. */
  static final int BLOCK_FRAMES = 8192;

  /** The bitrate LAME settled on, which can be lower than asked for at low sample rates. */
  int bitrateKbps() {
    return flags.brate;
  }

  /** The rate MP3 will actually use, after LAME's own resampling choice. */
  int outputSampleRate() {
    return flags.out_samplerate;
  }

  /**
   * Encodes [frames] sample frames of interleaved signed 16-bit PCM starting at [offset] (in
   * shorts). Mono input has [frames] shorts, stereo has twice that.
   */
  void write(short[] pcm, int offset, int frames) throws IOException {
    if (finished) throw new IllegalStateException("Encoder already finished");
    int done = 0;
    while (done < frames) {
      int count = Math.min(BLOCK_FRAMES, frames - done);
      int[] left = new int[count], right = new int[count];
      int base = offset + done * channels;
      for (int i = 0; i < count; i++) {
        // LAME's int entry points expect 32-bit full scale; a 16-bit sample is the top half.
        left[i] = pcm[base + i * channels] << 16;
        right[i] = channels == 2 ? pcm[base + i * channels + 1] << 16 : left[i];
      }
      int bytes = lame.lame_encode_buffer_int(flags, left, right, count, encoded, 0, encoded.length);
      if (bytes < 0) throw new IOException("MP3 encoder failed (" + bytes + ")");
      output.write(encoded, 0, bytes);
      done += count;
    }
  }

  /** Flushes the last frames. The encoder cannot be used afterwards. */
  void finish() throws IOException {
    if (finished) return;
    finished = true;
    int bytes = lame.lame_encode_flush(flags, encoded, 0, encoded.length);
    if (bytes < 0) throw new IOException("MP3 encoder failed to finish (" + bytes + ")");
    output.write(encoded, 0, bytes);
  }

  @Override
  public void close() {
    lame.lame_close(flags);
  }
}
