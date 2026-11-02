# M9: APK size reduction (2026-11-02)

Source changes only, at the owner's request. No APK/native/desktop build, Gradle
task, emulator run or release publication was performed. The published mobile.10
APK is the recorded baseline at **56,110,994 bytes**; no new size or percentage
saving has been measured.

## Changes

- Exclude the old egui/eframe/GLES renderer from normal mobile builds. Its exported
  `android_main` previously kept the legacy renderer reachable even though
  `RustActivity` launches Compose. Keep the prototype behind `legacy-ui`, and
  retain host unit-test coverage with a development-only egui dependency.
- Remove the Android GameActivity dependency. The current launcher extends
  `ComponentActivity` and does not use GameActivity. The shared Rust engine,
  account cipher, isolated JS evaluator, thumbnails and PCM metering stay in the
  normal JNI library. Legacy frame/lifecycle JNI probes remain callable without
  loading a renderer, preserving the Java declaration contract.
- Use an Android-only Cargo profile: size optimization (`s`), thin LTO, one codegen
  unit and no debug information. The packaging strip step and 16 KiB linker
  alignment remain. Preserve panic unwinding because JNI requests catch failures.
  Desktop release settings are unchanged.
- Enable release R8 minification and resource shrinking. Keep native bridge names
  and public MediaBridge callbacks used through JNI; preserve the persisted
  WorkManager worker name across updates. Preview Java/Kotlin builds remain
  debuggable and unminified.
- Extend APK inspection to require init/cancel exports and reject the old
  `android_main` entry point in distributed Compose APKs.

Both ARM64 and x86-64 remain in the universal APK. No languages, media formats,
themes, account state, saved library or notices are removed to reduce size.

## Evidence and remaining checks

Shell syntax and whitespace checks pass. Python TOML parsing and source comparison
verify the optional renderer dependencies, retained bridge implementation and
unchanged prototype implementation after the move. The existing offline Python
signing/publication tests pass (six tests). Cargo and rustfmt are unavailable in this workspace,
so Cargo metadata, formatting and compilation are not verified here.

The commit uses `[skip ci]` to honor the request to avoid builds after pushing.
These changes are unqualified until a later authorized build measures APK entry
sizes against the baseline and passes locked dependency resolution, Rust checks,
release R8/lint/JVM tests, signature/ABI/JNI/16 KiB inspection, and native runtime
checks. In particular, recheck encrypted account restore, JS challenges,
WorkManager download preparation, player layouts and the existing strict startup
deadlines: size optimization can affect runtime performance. Inspect the R8
mapping/usage reports if release-only behavior changes. Existing M9 physical/live
and parity gates remain open; this change creates no new release.
