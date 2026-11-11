package dev.ytui.android;

import static org.junit.Assert.*;
import static org.junit.Assume.assumeTrue;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import org.junit.Test;

public class Mp3EncoderTest {
  /** [seconds] of a 440 Hz tone, interleaved. The right channel is a fifth higher. */
  private static short[] tone(int rate, int channels, double seconds) {
    int frames = (int) (rate * seconds);
    short[] pcm = new short[frames * channels];
    for (int i = 0; i < frames; i++) {
      pcm[i * channels] = (short) (Math.sin(2 * Math.PI * 440 * i / rate) * 12000);
      if (channels == 2) pcm[i * channels + 1] = (short) (Math.sin(2 * Math.PI * 660 * i / rate) * 12000);
    }
    return pcm;
  }

  private static byte[] encode(int rate, int channels, double seconds) throws IOException {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    short[] pcm = tone(rate, channels, seconds);
    try (Mp3Encoder encoder = new Mp3Encoder(rate, channels, Mp3Encoder.BITRATE_KBPS, out)) {
      // Feed in uneven blocks, as a codec would.
      int frames = pcm.length / channels, at = 0, block = 1000;
      while (at < frames) {
        int count = Math.min(block, frames - at);
        encoder.write(pcm, at * channels, count);
        at += count;
        block += 733;
      }
      encoder.finish();
    }
    return out.toByteArray();
  }

  @Test
  public void stereoOutputStartsWithAValidFrameAndHasTheExpectedSize() throws Exception {
    byte[] mp3 = encode(44100, 2, 3);
    int[] frame = Mp3Policy.firstFrame(mp3, Math.min(mp3.length, 4096));
    assertNotNull("no MPEG frame at the start", frame);
    assertEquals(44100, frame[0]);
    assertEquals(2, frame[1]);
    assertTrue(Mp3Policy.plausibleSize(mp3.length, 44100L * 3, 44100, Mp3Encoder.BITRATE_KBPS));
  }

  @Test
  public void monoInputStaysMono() throws Exception {
    byte[] mp3 = encode(48000, 1, 2);
    int[] frame = Mp3Policy.firstFrame(mp3, Math.min(mp3.length, 4096));
    assertNotNull(frame);
    assertEquals(48000, frame[0]);
    assertEquals(1, frame[1]);
  }

  @Test
  public void lowSampleRatesUseMpeg2AndStayTheRightLength() throws Exception {
    byte[] mp3 = encode(22050, 2, 2);
    int[] frame = Mp3Policy.firstFrame(mp3, Math.min(mp3.length, 4096));
    assertNotNull(frame);
    assertEquals(22050, frame[0]);
    // MPEG-2 tops out at 160 kbit/s, so the encoder must report what it really used.
    ByteArrayOutputStream sink = new ByteArrayOutputStream();
    try (Mp3Encoder encoder = new Mp3Encoder(22050, 2, Mp3Encoder.BITRATE_KBPS, sink)) {
      assertTrue(encoder.bitrateKbps() <= 160);
      assertTrue(Mp3Policy.plausibleSize(mp3.length, 22050L * 2, 22050, encoder.bitrateKbps()));
    }
  }

  @Test
  public void anEncoderRefusesMoreThanTwoChannelsAndNonsenseRates() {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    try {
      new Mp3Encoder(44100, 6, 192, out);
      fail();
    } catch (IllegalArgumentException expected) {
    }
    try {
      new Mp3Encoder(100, 2, 192, out);
      fail();
    } catch (IllegalArgumentException expected) {
    }
  }

  @Test
  public void writingAfterFinishFails() throws Exception {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    try (Mp3Encoder encoder = new Mp3Encoder(44100, 2, 192, out)) {
      encoder.finish();
      try {
        encoder.write(new short[2], 0, 1);
        fail();
      } catch (IllegalStateException expected) {
      }
    }
  }

  /** An independent decoder (FFmpeg) must read what the encoder wrote: rate, channels, length. */
  @Test
  public void ffprobeReadsTheEncodedFile() throws Exception {
    String probe = which("ffprobe");
    assumeTrue("ffprobe is not installed", probe != null);
    File file = File.createTempFile("mp3-encoder-", ".mp3");
    try {
      Files.write(file.toPath(), encode(44100, 2, 3));
      String[] fields = run(probe, "-v", "error", "-select_streams", "a:0", "-show_entries",
          "stream=codec_name,sample_rate,channels:format=duration", "-of", "default=nw=1", file.getPath())
          .split("\n");
      String all = String.join(" ", fields);
      assertTrue(all, all.contains("codec_name=mp3"));
      assertTrue(all, all.contains("sample_rate=44100"));
      assertTrue(all, all.contains("channels=2"));
      double duration = Double.parseDouble(all.replaceAll(".*duration=([0-9.]+).*", "$1"));
      assertEquals(3.0, duration, 0.15);
      // A full decode must succeed without errors.
      String decodeErrors = run(which("ffmpeg"), "-v", "error", "-i", file.getPath(), "-f", "null", "-");
      assertEquals("", decodeErrors.trim());
    } finally {
      file.delete();
    }
  }

  /**
   * A real AAC-in-M4A file (what YouTube Music downloads are) goes through FFmpeg's decoder to PCM,
   * into the encoder, and the MP3 is read back by FFmpeg. The Android codec step (Mp3Decoder)
   * needs a device and is not covered here.
   */
  @Test
  public void aRealM4aSourceSurvivesTheRoundTrip() throws Exception {
    String ffmpeg = which("ffmpeg"), probe = which("ffprobe");
    assumeTrue("ffmpeg is not installed", ffmpeg != null && probe != null);
    File m4a = File.createTempFile("mp3-source-", ".m4a"), raw = File.createTempFile("mp3-source-", ".pcm"),
        mp3 = File.createTempFile("mp3-round-", ".mp3");
    try {
      run(ffmpeg, "-y", "-v", "error", "-f", "lavfi", "-i", "sine=frequency=330:duration=4:sample_rate=44100",
          "-f", "lavfi", "-i", "anoisesrc=d=4:c=pink:r=44100:a=0.05", "-filter_complex",
          "[0][1]amerge=inputs=2", "-c:a", "aac", "-b:a", "128k", m4a.getPath());
      run(ffmpeg, "-y", "-v", "error", "-i", m4a.getPath(), "-f", "s16le", "-ac", "2", "-ar", "44100", raw.getPath());
      byte[] bytes = Files.readAllBytes(raw.toPath());
      short[] pcm = new short[bytes.length / 2];
      java.nio.ByteBuffer.wrap(bytes).order(java.nio.ByteOrder.LITTLE_ENDIAN).asShortBuffer().get(pcm);
      try (java.io.OutputStream out = Files.newOutputStream(mp3.toPath());
          Mp3Encoder encoder = new Mp3Encoder(44100, 2, Mp3Encoder.BITRATE_KBPS, out)) {
        encoder.write(pcm, 0, pcm.length / 2);
        encoder.finish();
      }
      assertTrue(Mp3Policy.plausibleSize(mp3.length(), pcm.length / 2, 44100, Mp3Encoder.BITRATE_KBPS));
      String info = run(probe, "-v", "error", "-show_entries", "stream=codec_name,channels:format=duration",
          "-of", "default=nw=1", mp3.getPath()).replace("\n", " ");
      assertTrue(info, info.contains("codec_name=mp3") && info.contains("channels=2"));
      assertEquals("", run(ffmpeg, "-v", "error", "-i", mp3.getPath(), "-f", "null", "-").trim());
    } finally {
      m4a.delete();
      raw.delete();
      mp3.delete();
    }
  }

  private static String which(String tool) {
    for (String dir : System.getenv("PATH").split(File.pathSeparator)) {
      File candidate = new File(dir, tool);
      if (candidate.canExecute()) return candidate.getPath();
    }
    return null;
  }

  private static String run(String... command) throws Exception {
    Process process = new ProcessBuilder(command).redirectErrorStream(true).start();
    String out = new String(process.getInputStream().readAllBytes());
    process.waitFor();
    return out;
  }
}
