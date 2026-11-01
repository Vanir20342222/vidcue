# M9: mobile.10 content-first UI release

Version **0.2.0-alpha.12-mobile.10**, code **10**, packages the merged
[PR #29](https://github.com/Vanir20342222/ytui/pull/29) restyle from app source
**`686e3794b023acee4b7355c437bb84f887a42ac6`**. Android 8+, ARM64/x86-64,
package `dev.ytui.android.stable`.

[Download mobile.10 APK](https://github.com/Vanir20342222/ytui/releases/download/android-0.2.0-alpha.12-mobile.10/ytui-0.2.0-alpha.12-mobile.10.apk)
· [Release, checksum and public inspection](https://github.com/Vanir20342222/ytui/releases/tag/android-0.2.0-alpha.12-mobile.10).

## Included changes

Full-bleed video thumbnails and metadata rows, a slim branded header, unified
filters and quieter navigation follow the supplied visual brief. Settings use
full-page grouped rows and a theme radio dialog. Account and action sheets use
neutral surfaces; tablet search uses wide rows. Player chrome is updated while
Classic, vinyl, orbit, VU, existing Media3 commands, seek amounts and fullscreen
lifecycle remain available.

Fresh mobile installs default to neutral charcoal. Saved themes, imported
palettes and legacy aliases are preserved. Extraction, networking, cache and
account contracts are unchanged. [Implementation, decisions and retained
failures](CONTENT_FIRST_RESTYLE.md) distinguish visual scope from remaining
mobile acceptance gates.

## Exact-source qualification

- [Main Android](https://github.com/Vanir20342222/ytui/actions/runs/41174817530):
  **37 native passes**, zero failures, one explicit public-YouTube skip. Native
  input, account navigation, theme paint/import, offline playlist editing,
  decoder controls, player styles and fullscreen/return pass. No app ANR or
  Rust-panic markers. [Native evidence](https://github.com/Vanir20342222/ytui/actions/runs/41174817530/artifacts/13033111418).
- Main actual-window synthetic startup confirmations: progressive cold/prepared
  **1976/1083 ms**, indexed **1183 ms**, HLS cold/prepared **1512/1026 ms**.
  The strict 2000 ms limits are unchanged; these are synthetic/loopback results.
- [Main Rust](https://github.com/Vanir20342222/ytui/actions/runs/41174817483)
  and [secret scan](https://github.com/Vanir20342222/ytui/actions/runs/41174817627)
  pass. The equivalent qualified PR source records **367 Rust/media test passes**,
  31 explicit ignores, formatting, strict Clippy and release build success.
- The source review includes **204 after screenshots**, with the 156-capture
  phone/tablet/light/dark/theme matrix, 2x text and retained before captures.
  Original missing ZIP and inherited baseline state mismatches remain documented.
- The separately built signed release passes **20 release JVM tests**, zero
  failures or skips, and lint (zero errors; the existing warning inventory is
  retained). Source marker, non-debuggable package, increasing version code,
  permanent certificate, both Rust ABIs, ZIP/16 KiB ELF alignment and notice
  inventory are inspected before publication.

The owner's previous mobile.8 Tab S9 performance sample remains the physical
baseline. New physical UI/update, audible output, live YouTube, OEM background
behavior and the capability gaps in [PARITY.md](PARITY.md) remain separate.
This is a development preview.

## Build and updates

The existing supported overrides reproduce the release metadata from its source
tag; repository-default Preview metadata is earlier:

```bash
export YTUI_ANDROID_VERSION_CODE=10
export YTUI_ANDROID_VERSION_NAME=0.2.0-alpha.12-mobile.10
python3 tools/android/sign-local.py /private/path/to/recovered-identity
```

Keep private identity/recovery files outside Git as described in
[SIGNING.md](SIGNING.md). The certificate matches mobile.4–9. Install over the
stable-channel app without uninstalling to retain account, settings and library.
Older Preview package identities remain separate. Future public versions require
a higher version code and the same permanent identity.

## Publication and independent download

[Publisher 41350786300](https://github.com/Vanir20342222/ytui/actions/runs/41350786300)
passes, independently inspecting the already signed APK before publication
without rebuilding or re-signing it. Direct local upload encountered a 401 from
the GitHub upload endpoint; its empty draft was removed and the existing verified
publisher delivered the release. Signing credentials never entered GitHub or
the temporary APK transfer. The temporary private Drive copy was removed after
the independent public download verification.

The public APK was downloaded and its size, checksum, certificate, package and
version code/name verified against the signed build.

- APK SHA256: `e99d20d32f7e99d1c527e4d06c1d1163cdf77685324303147566cdac69d5fa0e`.
- Size: **56,110,994 bytes**.
- Certificate SHA256: `b3e9f1a70dd6597884d1dbba09eafad3d4d73ea62c4074a9b8cec1ac1e034848`.
