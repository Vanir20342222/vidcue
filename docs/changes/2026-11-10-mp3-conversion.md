# Convert offline audio to MP3 (M2 / M9)

- **Date:** 2026-11-10
- **PR:** not opened (branch pushed for review)
- **Milestone:** M2, M9
- **Platforms:** Android
- **Status:** JVM unit and FFmpeg round-trip verified; Android codec and folder paths not run on a device

## What it does

1. **Playlist menu.** The ⋮ menu on an offline playlist row has "Convert playlist to MP3". It converts
   every item with a complete offline audio copy, in playlist order. The action exists only there.
2. **Setting.** Settings > Folders > "Convert audio downloads to MP3" (off by default). While on, each
   newly finished audio download is saved as an MP3 in the Music folder instead of as the original file.
3. **Backfill.** Turning the setting on also converts audio that is already downloaded, in one
   background job with a "Converting to MP3: 3 of 12" status in Transfers. Items already MP3 or already
   converted are skipped, so the job can be repeated or interrupted without redoing work.

## What happens to originals

- The offline cache copy is never touched, so in-app offline playback keeps working.
- A file already saved to the Music folder is **replaced** by the MP3, and only after the MP3 is
  fully written, re-read (MP3 frame header and a size check) and the library entry points at it.
  Any failure before that leaves the original in place; a temporary file is always removed.
- An item with no saved file gets a new MP3 in the Music folder; playlist items go to the
  playlist's folder, named with the playlist position (`003 - Title.mp3`).
- Video downloads are never converted.
- A Music folder must be selected. Without one, the action reports "Choose a Music folder in
  Settings to save MP3 files" and does nothing. The setting works without "Save completed media to
  selected folders"; with both on, an audio download is saved once, as an MP3.

## How

- `Mp3Decoder` decodes the cached file to PCM with the platform MediaCodec (AAC, Opus, Vorbis).
- `Mp3Encoder` encodes with the jump3r LAME port, 192 kbit/s constant bitrate (lower where MPEG-2
  rates cap it). Constant bitrate needs no seek, so it streams straight to a document provider.
  It drives jump3r's core classes directly, because jump3r's `LameEncoder` wrapper needs javax.sound,
  which Android lacks.
- `Mp3Policy` holds the pure decisions (what to convert, naming, frame and size checks) and
  `Mp3Convert` is the WorkManager job (`ytui-mp3-*` unique work, so queued items survive a restart).
- Conversions run one at a time (2026-11-11 review), so an automatic item job and the backfill
  cannot convert the same download twice. Playlist items share the playlist's one folder through
  `StorageLocations.childFolder`, the same helper the folder saves use.
- Review check (2026-11-11): a 440 Hz left / 1 kHz right test tone encoded and decoded with FFmpeg
  keeps each tone on its own channel at 97% of its input level, so channel order and 16-bit to
  LAME scaling are right.

## Licence

jump3r is **LGPL-2.1** (LAME is LGPL). It is linked as a normal replaceable Gradle dependency and is
listed in `THIRD_PARTY_NOTICES.md`. The project is MIT; shipping an LGPL library in the APK keeps
the usual LGPL obligations (licence notice, ability to relink with a modified jar).

## Verification

- `Mp3EncoderTest` (JVM): stereo, mono, 22.05 kHz; frame header, size, and FFmpeg decode of the
  output without errors; a real AAC-in-M4A made by FFmpeg round-tripped through the encoder.
- `Mp3PolicyTest`, `Mp3SettingTest`: skip/backfill selection, naming, header and size checks, and the
  setting's default (off) and persistence.
- Not run: `Mp3Decoder` (MediaCodec) and `Mp3Convert` (document provider writes, WorkManager) need a
  device or emulator.
