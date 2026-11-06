# Retain APK inputs while pruning compiler caches (M9)

- **Date:** 2026-11-04
- **Milestone:** M9
- **Platforms:** Android build tooling
- **Status:** fixture verified

## What it does

The new opt-in small-disk build mode used `cargo clean --target` after copying each
JNI library. The local signed build compiled both ABIs, but that cleanup removed
the whole target directory, including the JNI copies, before notices/packaging.
Only the selected cross-compilation cache is now removed explicitly. Shared host
artifacts and APK inputs remain available.

## Evidence

An offline end-to-end build-script fixture reproduced the exact missing
`target/android-dependencies.json` failure with pruning enabled. After the fix,
the fixture verifies both pruning modes, both copied JNI libraries, retained host
cache, dependency metadata, license/notices and the Gradle packaging invocation.
All 29 Android release-helper tests pass. Shell syntax and diff checks pass.
The fixture explicitly selects its fake NDK so a hosted runner's preconfigured
`ANDROID_NDK_HOME` cannot replace the disposable tools; the inherited-NDK failure
was reproduced locally and corrected without changing assertions.

## Not verified and limits

The signed APK build is restarted from the final merged source. No key, package,
version, Android app logic or runtime feature changes in this fix. Physical/live
acceptance remains separate from packaging.

## For the plan and parity tables

- Android release tooling: small-disk target pruning retains both packaged JNI
  ABIs and shared host artifacts; offline regression verified.
