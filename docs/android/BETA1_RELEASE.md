# Vidcue Android 0.3.0-beta.1

Release preparation for [PR #82](https://github.com/Vanir20342222/ytui/pull/82).
The APK has not been published: the owner confirmed on 2026-11-04 that the existing
private signing setup is unavailable. Version defaults are `0.3.0-beta.1`, code
`30001`. Desktop release versions are unchanged.

## Included changes

- Vidcue app name, shared crate names and native library; the Android package stays
  `dev.ytui.android.stable` and existing account, settings and library stay in place.
- Signed releases can check for updates in About and offer Update now, Later or
  Skip this version. Downloads are checked against their checksum, package, version
  and signing certificate before Android asks the user to confirm installation.
- The release publisher requires increasing version codes and the existing stable
  certificate. Future APKs carry the marker used by the update check. Once that
  channel starts, the separate debug Preview publisher refuses further publication.
- All changes already landed on main before this PR remain included, including
  nested-playlist preparation, lyric sweep, artwork-driven visualizers and scrolling
  regressions. Other open PRs are outside this release preparation.

## Finish publication when signing is available

1. Use the final merged main revision whose Android, Rust and Secret scan push
   workflows all pass. Keep that exact clean checkout for the signing build and
   publisher; if main changes, qualify and build the new revision instead.
2. Recover the original signing directory privately as described in [SIGNING.md](SIGNING.md).
   The helper still expects the existing `ytui-stable.jks` and `credentials.json`.
3. On that trusted machine, with the Android SDK/NDK and Rust targets available:

   ```bash
   export YTUI_ANDROID_VERSION_NAME=0.3.0-beta.1
   export YTUI_ANDROID_VERSION_CODE=30001
   python3 tools/android/sign-local.py /private/path/to/recovered-identity
   ```

4. Publish the resulting `apps/android/app/build/outputs/apk/release/app-release.apk`
   through **Publish locally signed Android APK**, using the same code/name and
   its SHA256. Only the finished APK goes through the temporary transfer service.
   The workflow checks source, certificate, package, version, both ABIs, ZIP and
   16 KiB ELF alignment before creating `android-0.3.0-beta.1` as a prerelease.
5. Independently download the public APK, compare its checksum/certificate/version,
   and install over the existing owner-signed alpha stable app. Confirm settings,
   account and library retention. The first alpha-to-beta installation is manual;
   in-app update support starts with beta.1 and can be exercised with beta.2.

## Qualification limits

Beta denotes an early testing release. Physical installation/update acceptance,
live YouTube/account behavior, audible output, hardware playback and OEM background
behavior remain open. No signed beta APK, emulator run, physical-device test or
live-service acceptance is claimed by this preparation. See the PR's verification
table and the [completion change note](../changes/2026-11-04-vidcue-beta-preparation.md)
for checks actually run on the final source.
