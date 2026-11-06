# Remove the retired egui Android UI (M9, AR-21)

- **Date:** 2026-11-05
- **Milestone:** M9
- **Platforms:** Android, shared engine (`crates/vidcue-mobile`)
- **Status:** implemented; host unit tests, Android-target Rust build and Gradle compile/JVM tests verified. No emulator or device run.

## What it does
The Android app has used native Jetpack Compose since the 2026-10-28 stack change
([NATIVE_UI_MIGRATION.md](../android/NATIVE_UI_MIGRATION.md)). `RustActivity` is an androidx
`ComponentActivity`, so the egui/eframe `android_main` entry point never ran, but its code and
dependencies still shipped in `libvidcue_mobile.so` and the APK (review finding AR-21 in
[ANDROID_REVIEW_2026-11-05.md](../audit/ANDROID_REVIEW_2026-11-05.md)). This change removes that
dead code. App behaviour does not change.

Removed:
- `crates/vidcue-mobile/src/ui.rs` and `src/ui/lyrics.rs` (egui `MobileApp`, lyric editor view,
  `Effect` queue, `linked_media`, `lrc_text`).
- From `src/platform.rs`: `android_main`, the `eframe::App` adapter, the GLES external-texture
  video painter (`Video`, `paint_video`), `bridge_call`, and the JNI natives `frameAvailable`,
  `uiGeneration`, `uiShutdowns`.
- Rust dependencies `egui`, `eframe`, `android-activity` (GameActivity) and `glow`; `Cargo.lock` drops
  from 649 to 537 packages. The `[patch.crates-io]` overrides and the vendored copies
  `vendor/winit` (0.30.13) and `vendor/eframe` (0.33.3) are removed: they patched only Android
  activity teardown for this UI, and without `eframe` in the graph they would be unused patches.
- Java: `NativeBridge.frameAvailable/uiGeneration/uiShutdowns`; `MediaBridge.videoFrame`,
  `releaseVideo` and their `surface`/`textureId`/`frameReady` state.
- Gradle `androidx.games:games-activity`, the manifest `android.app.lib_name` meta-data (only
  GameActivity read it; `NativeBridge` loads the library with `System.loadLibrary`), and the
  GameActivity line in the shipped `ANDROID-NOTICES.txt`.

Kept, because the Compose app or engine still uses them:
- `platform::initialize_bridge` (called from `NativeBridge.init`) with the `MediaBridge.evaluate`
  JS-solver callback, and `platform::account_cipher` (`MediaBridge.sealAccount`/`openAccount`,
  used by `account.rs`).
- `MediaBridge.snapshot`/`execute` (called from `RustRepository.kt` and instrumentation tests),
  `uiMode` (`PlaybackNavigationTest`), `notifyUi`, `videoFrames` (incremented by
  `PlaybackService`, read by instrumentation tests), `videoWidth`/`videoHeight`, and the
  `texture` field, which is now always null but is still read by `PlaybackService` (owned by
  another fix group; its resize branch can be removed there later).
- `RustActivity.generations`/`shutdowns` (used by `MediaRuntimeTest`).
- `crate::lyrics::Document` and the engine lyric store; their behaviour stays covered by
  `tests/engine.rs::local_lyrics_*` and `vidcue-providers` lyric tests.

No logic in `ui.rs` or `ui/lyrics.rs` was used by the engine, so nothing needed to move.

### Removed tests
All exercised only the removed egui `MobileApp` state machine, its rendering, or the egui lyric
editor view. None covered engine or Compose behaviour.

| Test (crate `vidcue-mobile`) | Why removed |
| --- | --- |
| `ui::tests::pasted_playlist_links_expand_but_watch_links_keep_single_video_intent` | Tested egui-only `linked_media`; Compose uses `RustRepository.linked` (Kotlin). URL normalisation and playlist detection remain tested in `vidcue-core` `media.rs`. |
| `ui::tests::a_pending_download_keeps_its_requested_quality_when_settings_change` | egui reply handling and `Effect::Download`. |
| `ui::tests::stale_playback_and_account_replies_cannot_start_media` | egui reply generation checks and `Effect::Play`. |
| `ui::tests::feed_retry_respects_server_cooldown_preserves_content_and_is_bounded` | egui feed retry state. |
| `ui::tests::modal_geometry_respects_system_bars_and_download_cancel_restores_preferences` | egui safe-area layout and modal state. |
| `ui::tests::android_back_closes_overlays_before_backgrounding` | egui overlay/back handling. |
| `ui::tests::back_restores_feed_position_rejects_late_replies_and_disconnect_clears_private_history` | egui feed navigation history. |
| `ui::tests::navigation_retains_recent_pages_with_a_bounded_memory_and_depth_budget` | egui `Feed` history budget. |
| `ui::tests::dismissing_a_loading_queue_cannot_be_undone_by_its_reply` | egui queue sheet state. |
| `ui::tests::every_music_style_renders_and_pause_reduced_motion_and_cancel_preserve_state` | egui music-player rendering. |
| `ui::tests::collection_continuation_is_scoped_to_the_selected_finite_music_or_radio_feed` | egui `MobileApp::collection_context`; Compose builds its own context in `RustRepository.kt`. |
| `ui::tests::phone_tablet_and_all_theme_layouts_render_without_network` | egui headless layout rendering. |
| `ui::tests::delayed_account_status_cannot_overwrite_a_newer_import_or_disconnect` | egui status-line handling; engine account-intent ordering stays covered in `tests/engine.rs`. |
| `ui::tests::phone_navigation_responds_to_touch_sized_pointer_input` | egui pointer input. |
| `ui::lyrics::tests::lyric_editor_and_synced_view_render_on_phone_and_tablet_in_all_themes` | egui lyric window rendering. |
| `ui::lyrics::tests::timing_offsets_and_timestamp_roundtrip_preserve_track_identity` | egui lyric `View` offset and `lrc_text` formatting; `Document::track` and `Track::active_line` stay covered by engine and provider tests. |
| `ui::lyrics::tests::dismissed_or_superseded_lyrics_cannot_be_reopened_by_late_requests_or_imports` | egui lyric window request lifecycle. |

Earlier egui backend/headless evidence in the shared docs is left as written; it records what
those tests showed at the time.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Format | `cargo fmt --all -- --check` | pass | unit |
| Lint | `cargo clippy -p vidcue-mobile --all-targets --locked -- -D warnings` | pass | unit |
| Mobile tests | `cargo test -p vidcue-mobile --locked` | 36 lib + 17 engine tests pass (was 53 + 17; 17 egui tests removed) | unit |
| Workspace tests | `cargo test --workspace --locked` | pass; 450 passed, 0 failed, 31 ignored across 28 test binaries (run with debuginfo off after a disk-full link failure on the first attempt) | unit |
| Android-target Rust | `cargo build -p vidcue-mobile --target aarch64-linux-android --release` (NDK 27.2.12479018), before and after | both pass | build |
| Native library size | `llvm-strip --strip-unneeded libvidcue_mobile.so` (arm64) | 19,922,192 → 15,100,728 bytes stripped (−4,821,464, −24.2%); unstripped 27,904,600 → 21,386,160 | build |
| Exported symbols | `llvm-nm -D --defined-only` | `android_main`, `GameActivity_onCreate`, `Java_com_google_androidgamesdk_GameActivity_initializeNativeCode` and the three removed `NativeBridge` natives are gone; the 12 natives `NativeBridge.java` declares remain | build |
| Android app | `gradlew :app:compileDebugKotlin :app:compileDebugAndroidTestKotlin :app:testDebugUnitTest` | pass; 168 JVM tests, 0 failures (same count as before: no JVM test touched removed code) | unit |

## Not verified and limits
- No emulator or physical-device run in this session. Instrumentation tests (`MediaRuntimeTest`,
  `CatalogUiTest`, `ContrastUiTest`, `PlaybackNavigationTest`) were compiled, not run.
- The x86_64 Android library was not rebuilt locally; CI builds it.
- `MediaBridge.texture` stays as an always-null field until `PlaybackService` drops its read.

## For the plan and parity tables
- docs/android/README.md, "Repository structure": `crates/vidcue-mobile` is the "Rust Android
  engine, JNI boundary and PCM visualizer metering" (no touch UI or GLES video). Remove the
  `vendor/winit` and `vendor/eframe` bullets (removed 2026-11-05, AR-21).
- docs/android/VALIDATION.md: "2026-11-05 (AR-21): the retired egui UI, eframe/glow/GameActivity
  and the unused natives `frameAvailable`, `uiGeneration`, `uiShutdowns`, `videoFrame` and
  `releaseVideo` were removed. Earlier egui headless results remain historical evidence only."
