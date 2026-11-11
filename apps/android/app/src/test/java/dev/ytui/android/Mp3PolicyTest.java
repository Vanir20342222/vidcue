package dev.ytui.android;

import static org.junit.Assert.*;

import java.util.Arrays;
import java.util.List;
import org.junit.Test;

public class Mp3PolicyTest {
  private static Mp3Policy.Candidate item(String id, boolean audio, String savedAs, boolean done) {
    return new Mp3Policy.Candidate(id, audio, savedAs, done);
  }

  @Test
  public void mp3NamesAndMimeTypesAreRecognised() {
    assertTrue(Mp3Policy.isMp3("audio/mpeg"));
    assertTrue(Mp3Policy.isMp3("content://x/document/primary%3AMusic%2FSong.MP3"));
    assertFalse(Mp3Policy.isMp3("content://x/document/primary%3AMusic%2FSong.m4a"));
    assertFalse(Mp3Policy.isMp3(null));
    assertFalse(Mp3Policy.isMp3(""));
  }

  @Test
  public void alreadyMp3VideoAndFinishedItemsAreSkipped() {
    assertTrue(Mp3Policy.needsConversion(item("a", true, null, false)));
    assertTrue(Mp3Policy.needsConversion(item("a", true, "content://x/Song.m4a", false)));
    assertFalse("already an MP3", Mp3Policy.needsConversion(item("a", true, "content://x/Song.mp3", false)));
    assertFalse("converted before", Mp3Policy.needsConversion(item("a", true, null, true)));
    assertFalse("video", Mp3Policy.needsConversion(item("a", false, null, false)));
    assertFalse("second half of a split download", Mp3Policy.needsConversion(item("a-audio", true, null, false)));
  }

  @Test
  public void backfillPicksOnlyWhatIsLeftInOrderWithoutRepeats() {
    List<Mp3Policy.Candidate> items =
        Arrays.asList(
            item("one", true, "content://x/One.m4a", false),
            item("two", true, "content://x/Two.mp3", false),
            item("three", false, null, false),
            item("four", true, null, true),
            item("five", true, null, false),
            item("one", true, "content://x/One.m4a", false),
            item("five-audio", true, null, false));
    assertEquals(Arrays.asList("one", "five"), Mp3Policy.backfill(items));
    assertTrue(Mp3Policy.backfill(java.util.Collections.emptyList()).isEmpty());
  }

  @Test
  public void automaticConversionNeedsTheSettingAndAudio() {
    assertTrue(Mp3Policy.automatic(true, true));
    assertFalse(Mp3Policy.automatic(false, true));
    assertFalse(Mp3Policy.automatic(true, false));
  }

  @Test
  public void convertedFilesKeepThePlaylistOrderInTheirName() {
    assertEquals("Song.mp3", Mp3Policy.fileName("Song", -1));
    assertEquals("003 - Song.mp3", Mp3Policy.fileName("Song", 2));
    assertFalse(Mp3Policy.fileName("a/b", -1).contains("/"));
  }

  @Test
  public void frameHeaderParsingAcceptsMpegLayer3AndSkipsId3() {
    // MPEG-1 Layer III, 128 kbit/s, 44.1 kHz, joint stereo.
    byte[] frame = {(byte) 0xff, (byte) 0xfb, (byte) 0x90, 0x44};
    assertArrayEquals(new int[] {44100, 2}, Mp3Policy.firstFrame(frame, 4));
    byte[] mono = {(byte) 0xff, (byte) 0xfb, (byte) 0x90, (byte) 0xc4};
    assertArrayEquals(new int[] {44100, 1}, Mp3Policy.firstFrame(mono, 4));
    // ID3v2 tag of 6 payload bytes, then the frame.
    byte[] tagged = new byte[10 + 6 + 4];
    tagged[0] = 'I';
    tagged[1] = 'D';
    tagged[2] = '3';
    tagged[9] = 6;
    System.arraycopy(frame, 0, tagged, 16, 4);
    assertArrayEquals(new int[] {44100, 2}, Mp3Policy.firstFrame(tagged, tagged.length));
  }

  @Test
  public void notMp3DataIsRejected() {
    assertNull(Mp3Policy.firstFrame(new byte[] {0, 0, 0, 0}, 4));
    assertNull("m4a header", Mp3Policy.firstFrame(new byte[] {0, 0, 0, 0x18, 'f', 't', 'y', 'p'}, 8));
    assertNull("too short", Mp3Policy.firstFrame(new byte[] {(byte) 0xff, (byte) 0xfb}, 2));
    // Layer II (bits 01 at the layer field) is not Layer III.
    assertNull(Mp3Policy.firstFrame(new byte[] {(byte) 0xff, (byte) 0xfd, (byte) 0x90, 0x44}, 4));
    // Free/invalid bitrate index.
    assertNull(Mp3Policy.firstFrame(new byte[] {(byte) 0xff, (byte) 0xfb, (byte) 0xf0, 0x44}, 4));
  }

  @Test
  public void sizeCheckAcceptsTheExpectedLengthAndRejectsTruncationOrEmpty() {
    long frames = 44100L * 100; // 100 s at 192 kbit/s is 2 400 000 bytes
    assertTrue(Mp3Policy.plausibleSize(2_400_000, frames, 44100, 192));
    assertTrue(Mp3Policy.plausibleSize(2_450_000, frames, 44100, 192));
    assertFalse("truncated", Mp3Policy.plausibleSize(1_200_000, frames, 44100, 192));
    assertFalse("empty", Mp3Policy.plausibleSize(0, frames, 44100, 192));
    assertFalse("no audio", Mp3Policy.plausibleSize(2_400_000, 0, 44100, 192));
  }
}
