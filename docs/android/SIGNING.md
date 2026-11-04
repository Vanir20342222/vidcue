# Android APK signing and updates

The current [mobile.10 APK and public inspection](https://github.com/Vanir20342222/ytui/releases/tag/android-0.2.0-alpha.12-mobile.10) use the permanent certificate below. Source `686e3794b023acee4b7355c437bb84f887a42ac6` passed exact-main checks; the public APK hash was independently verified. [Qualification and release build metadata](MOBILE10_RELEASE.md).

Development APKs from the Android workflow use debug identities. New debug
builds use `dev.ytui.android.preview` and the launcher label **vidcue Preview**.
This allows installing them alongside older `dev.ytui.android` development APKs
without uninstalling or deleting existing private data. State and Keystore
sessions are separate; no cross-app private data import is attempted. Release
builds now use `dev.ytui.android.stable` and the normal **vidcue** launcher label.
This one-time owner-signed channel avoids conflicts with both earlier disposable
package identities. Keep existing installations: their private data is untouched.
The stable channel has separate account/settings/cache and will need sign-in and
folder selection once. Every subsequent stable update must use the same owner key.

Android requires the same signing identity to update an installed package while retaining its
private account/library/cache. A different key requires a separate application
identity or uninstalling, which removes private state. Never uninstall a wanted
installation just to bypass this check.

The manual **Android signed APK** workflow builds a non-debuggable release APK
with an owner-supplied identity. It runs only from `main`, requires successful
Android, Rust and Secret scan runs for that exact revision, and uploads an APK
and inspection report. It does not publish a GitHub Release or mark M9 complete.
Physical-device/live and capability gates in [PARITY.md](PARITY.md) remain.

## Beta channel: every published APK updates the installed beta

From the beta on, every APK we publish must install **over** the installed beta, keeping the
user's account, settings and library. Android allows that only when the new APK has:

| Rule | How it is enforced |
| --- | --- |
| Same package, `dev.ytui.android.stable` | `verify-local-release.py` checks the package name. Debug Preview APKs (`.preview`) are never offered. |
| Same signing key (certificate SHA256 above) | `verify-local-release.py` checks the certificate. The app also compares the release's marker with its own certificate. |
| Strictly higher version code | `release_channel.require_update` compares it with **every** published release (markers plus the codes in older notes). Release builds also refuse to use the default code. |

**Version codes.** Use `python3 tools/android/release_channel.py code 0.3.0-beta.1`:

- The formula is `MAJOR·1,000,000 + MINOR·10,000 + PATCH·100 + stage`. The stage is N for `beta.N`, 50+N for `rc.N` and 99 for the final release.
- So `0.3.0-beta.1` → 30001, `0.3.0-beta.2` → 30002, `0.3.0-rc.1` → 30051, `0.3.0` → 30099 and `0.3.1-beta.1` → 30101.
- Every code is above the alpha codes already published (10–15), so the first beta installs over the current alpha stable app too.

**In-app updates.**

- Each locally signed release's notes end with an invisible `vidcue-android-update` marker. The marker records the package, version code and name, APK file name, APK SHA256, certificate SHA256 and minimum SDK.
- Vidcue (signed builds only) reads the GitHub release list when it opens, at most every six hours, and shows **Update now · Later · Skip this version**.
- *Update now* downloads the APK and checks the SHA256, package, version code and certificate. It then hands the APK to Android's installer, which asks the user to confirm.
- The first time, Android asks to allow installs from Vidcue.
- About → *Check for updates automatically* turns the startup check off. About → *Check for updates* checks on demand.
- Releases without the marker (all alphas so far) are never offered.

**CI Preview releases stop at the beta.** CI Preview APKs are debug-signed under another package, so they can never update the beta. *Release verified Android Preview* therefore refuses to publish once any release carries the marker. From then on, publish only through *Publish locally signed Android APK*.

Release checklist for each beta/rc/final:

1. Pick the name, then `export YTUI_ANDROID_VERSION_NAME=0.3.0-beta.1` and
   `export YTUI_ANDROID_VERSION_CODE=$(python3 tools/android/release_channel.py code "$YTUI_ANDROID_VERSION_NAME")`.
2. Build and sign locally with `tools/android/sign-local.py` (below).
3. Run *Publish locally signed Android APK* with the same code and name. It refuses a code that is not higher than every published release, and adds the update marker.
4. On a device with the previous beta installed, open About → Check for updates and install. Confirm that the account, settings and library are still there. This physical check is not automated.

Never lose the signing key: without it no further update can reach installed betas.

## Current delivery: private local signing, no GitHub secrets

The owner rejected GitHub secret setup on 2026-10-28. The stable signing identity
now exists outside the repository in private local storage, with an encrypted
recovery archive backed up privately to the owner. Nothing needs to be pasted
into GitHub settings or chat. Only signed APKs and public certificate/checksum
reports may be published. The public certificate SHA256 is:

```text
B3:E9:F1:A7:0D:D6:59:78:84:D1:DB:BA:09:EA:FA:D3:D4:D7:3E:A6:2C:40:74:A9:B8:CE:C1:AC:1E:03:48:48
```

The recovery archive is `age`-encrypted to the owner's existing GitHub ED25519
SSH public key (fingerprint
`SHA256:c4Dmc5jTesW8obtpVyLphK6HgDSqHTCBPKw4v1QQMHg`). Its location is sent
privately, never committed. Recovery requires the corresponding private SSH key
on a trusted local machine; this workspace does not have that private key.
Keep the archive and SSH identity backed up: losing either prevents recovery.

To recover locally, install `age`, save the private archive, and run:

```bash
install -d -m 700 "$HOME/.local/share/ytui-signing"
umask 077
age --decrypt -i "$HOME/.ssh/id_ed25519" vidcue-android-signing-backup.age |
  tar -xf - -C "$HOME/.local/share/ytui-signing"
export YTUI_ANDROID_VERSION_CODE=10
export YTUI_ANDROID_VERSION_NAME=0.2.0-alpha.12-mobile.10
python3 tools/android/sign-local.py "$HOME/.local/share/ytui-signing"
```

The supported version inputs above reproduce the current release metadata;
the source tag may have earlier default Preview metadata. Increase the version
code for future public updates. The helper loads the recovered passwords privately into the build environment,
refuses an identity inside Git or one readable by other users, and builds both
Rust ABIs plus release unit/lint checks. It never prints the credentials. Do not
run with shell tracing, dump the process environment or upload the recovery files.
Future signing can run locally using this same archive; a cloud session cannot
recover it without access to the owner's SSH private key. This trades automatic
CI signing for keeping private signing credentials outside GitHub.

The manual **Publish locally signed Android APK** workflow transfers only a
finished APK through a temporary HTTPS file-service URL. It requires successful
Android/Rust/Secret scan runs for the exact `main` revision, verifies the source
marker, checksum, stable certificate, release package/version, both ABIs and
16 KiB alignment, then publishes a development prerelease. It never receives a
keystore, password or recovery archive, rebuilds the APK, or changes its signature.
Temporary APK transfers are masked in logs; no public sharing of Drive files is
needed. The old debug Preview publisher is unrelated to this stable channel.

A separate stable package is necessary because the old Preview's private key is
unavailable. The new stable key cannot update that old package in place. No
uninstall is required. Physical-device stable-update acceptance remains open.

## CI debug-signed Preview releases

The manual **Release verified Android Preview** workflow publishes a prerelease
of the current `main` revision when no owner-signed build is available. It runs
only from `main` and takes the version name the tested APK was built with (the
default is in `apps/android/app/build.gradle`). It requires successful
Android, Rust and Secret scan runs for that exact revision, downloads the exact
`vidcue-android-<sha>` APK that the Android run built and inspected, and checks
its CI inspection report, checksum, both ABIs, notices, source revision and
version name (`tools/android/prepare-preview-release.py`, with offline tests).
It never rebuilds or re-signs the APK, refuses an existing tag, and lists the
merged changes since the last `android-0.2.0-alpha.12-mobile.*` release.

These releases are named `android-preview-<version>`. The APK is
`dev.ytui.android.preview` (**vidcue Preview**) with a CI debug certificate, so it
neither updates the stable app nor, normally, an earlier Preview, because each
CI runner generates its own debug key. It is not the in-place update channel
for `dev.ytui.android.stable`; that remains the locally signed flow above.

## Optional: owner-managed GitHub signing

The existing **Android signed APK** workflow remains available if the owner
later chooses CI signing. It is not required by the current local workflow.

## Prepare the identity privately

Generate a release keystore on a trusted local machine, outside the repository,
and retain an encrypted backup of both the file and its passwords. For example,
this command prompts for passwords:

```bash
keytool -genkeypair -keystore /private/path/vidcue-release.jks -storetype JKS \
  -alias vidcue -keyalg RSA -keysize 3072 -validity 10000
```

Losing this
identity prevents future updates to installations signed with it. Do not use the
disposable key used to test the build, and do not commit a keystore or password.

In the repository's `android-release` GitHub Environment, configure these secrets:

| Secret | Value |
| --- | --- |
| `YTUI_ANDROID_KEYSTORE_BASE64` | The keystore encoded as one base64 line |
| `YTUI_ANDROID_KEY_ALIAS` | Its key alias |
| `YTUI_ANDROID_STORE_PASSWORD` | Keystore password |
| `YTUI_ANDROID_KEY_PASSWORD` | Key password, possibly the same as store password |

Use GitHub's private secret entry or `gh secret set --env android-release` with
standard input. Never paste the values into an issue, chat, command argument or
workflow log. Configure environment protection and restrict repository write
access to the people allowed to sign builds. Signing requires no Google API key,
YouTube account or browser cookies.

## Build and verify

After the selected main revision's checks pass, dispatch **Android signed APK**
from `main` with an increasing `version_code` (1–2100000000) and a version name
such as `0.2.0-alpha.12-mobile.4`. Download its `vidcue-android-signed-<code>-<sha>`
artifact. The report includes the APK SHA256, public certificate fingerprints,
source revision, both ABIs, 16 KiB alignment and license checks. Confirm the
certificate matches the previous signed APK before installing an update.

For a local release build, privately set `YTUI_ANDROID_KEYSTORE` to the keystore
path and the three alias/password environment variables above. Set
`YTUI_ANDROID_VERSION_CODE`, `YTUI_ANDROID_VERSION_NAME`, and run:

```bash
YTUI_ANDROID_VARIANT=release ./tools/android/build.sh
./tools/android/inspect-apk.sh apps/android/app/build/outputs/apk/release/app-release.apk
```

No release build falls back to debug signing or silently produces an unsigned
APK. The keystore path is read locally; passwords are not written to Gradle
properties. Rust libraries/notices build before the signing secrets are loaded.
The CI copy exists only in the runner's private temporary directory
and is removed when the signing step ends. Only the APK and public inspection
report are uploaded. About in the app shows its version/code and Rust revision.

The stable owner key has signed a local release build successfully. Final
exact-source device qualification and physical update acceptance are separate
gates; a valid certificate alone does not establish them.
