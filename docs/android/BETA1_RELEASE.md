# Vidcue Android 0.3.0-beta.1

Version `0.3.0-beta.1`, code `30001`, starts the permanent Vidcue Android update
channel. The owner clarified on 2026-11-04 that every later release must update
this beta; compatibility with the installed alpha is not required.

The package is `dev.vidcue.android.stable`. It installs alongside older alpha and
Preview apps, starting with separate app-private account, settings and library
state. Every subsequent beta, RC and final release keeps this package and signing
certificate, updating the beta in place. About → Check for updates offers them.

## Changes

- Vidcue branding and the renamed shared Rust/JNI library from PR #82.
- Verified in-app APK updates with Update now, Later and Skip this version.
- A permanent beta identity backed up privately with SSH-key encryption.
- Publication checks exact source, checksum, package, certificate, both ABIs,
  16 KiB alignment and increasing version codes. Future releases must also match
  the certificate recorded in earlier beta-channel markers.
- Debug Preview publication refuses beta/RC/final names and stops after the beta.

## Build and publish

1. Use a clean current-main revision whose exact Android, Rust and Secret scan
   push workflows all pass. Recover the beta identity using [SIGNING.md](SIGNING.md).
2. Set `YTUI_ANDROID_VERSION_NAME=0.3.0-beta.1` and `YTUI_ANDROID_VERSION_CODE=30001`.
   Run `python3 tools/android/sign-local.py /private/path/to/beta-identity`.
3. Transfer only the finished APK through a short-lived HTTPS file-service URL.
   Keep the signing key and passwords out of Git and Actions.
4. Run **Publish locally signed Android APK** on that revision with the APK URL,
   checksum, version and code. Alternatively, write a local JSON handoff containing
   `version_name`, integer `version_code`, `sha256`, `apk_url` and the full `source`
   revision. Run `python3 tools/android/publish_request.py create /private/transfer.json`
   which returns a temporary branch name and metadata commit. Push that commit to
   `refs/heads/<returned-branch>`. The commit must have the qualified source as its
   only parent and exactly the same file tree; the workflow verifies both.
5. The workflow masks the transfer URL, requires successful checks for exact
   current main, verifies the finished APK without rebuilding and publishes
   `android-0.3.0-beta.1`. It deletes the temporary handoff branch afterward. The
   permanent release tag points only at source. The expiring transfer URL grants
   read access only to the APK being released publicly, never to signing material.
6. Independently download and inspect the public APK. Install beta.1 as a new app.
   When beta.2 exists, accept its actual in-place update and data retention on a device.

## Qualification limits

Beta is an early testing release. Physical updates, live YouTube/account behavior,
audible output, hardware playback and OEM background behavior remain open.
Unit/build/APK inspection does not establish those results. See the
[channel evidence](../changes/2026-11-04-android-permanent-beta-channel.md) and
[original integration evidence](../changes/2026-11-04-vidcue-beta-preparation.md).
