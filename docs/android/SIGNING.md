# Android signing and permanent beta updates

The owner clarified on 2026-11-04 that beta.1 starts a new installation channel.
All subsequent beta, RC and final releases must update that beta in place.
Compatibility with installed alphas is not required.

## Permanent identity

| Property | Fixed value |
| --- | --- |
| Application package | `dev.vidcue.android.stable` |
| Public certificate SHA256 | `d3c9001eb3b22b31549f515f2d976000d3280ae8d5d0fc517bd03be946ddfd20` |
| Keystore filename | `vidcue-beta.jks` |
| Key alias | `vidcue-beta` |

The shared pin is in `tools/android/release_channel.py`. Never generate a replacement
key for this established channel. Signing and publishing reject another certificate,
a different package or a non-increasing code. The publisher also compares the
certificate with earlier marked releases in this channel.

Java/JNI classes stay in `dev.ytui.android`; their namespace need not match the
application ID. Debug builds use `dev.vidcue.android.preview` and cannot update
the beta. Old alpha apps remain installed independently, with their private data intact.

## Private recovery

The one-time beta key was created outside Git in an owner-only directory. Its
encrypted archive was uploaded to owner-only Google Drive and read back to compare
bytes. The private Drive link is given to the owner in chat, never committed here.

Archive: `Vidcue Android beta signing backup 2026-11-04.tar.age`.
Encryption recipient: the owner's existing GitHub ED25519 SSH public key,
fingerprint `SHA256:c4Dmc5jTesW8obtpVyLphK6HgDSqHTCBPKw4v1QQMHg`.

On a trusted machine with that SSH private key and `age`:

```bash
install -d -m 700 "$HOME/.local/share/vidcue-signing/beta"
umask 077
age --decrypt -i "$HOME/.ssh/id_ed25519" "Vidcue Android beta signing backup 2026-11-04.tar.age" | tar -xf - -C "$HOME/.local/share/vidcue-signing/beta"
export YTUI_ANDROID_VERSION_NAME=0.3.0-beta.1
export YTUI_ANDROID_VERSION_CODE=30001
python3 tools/android/sign-local.py "$HOME/.local/share/vidcue-signing/beta"
```

The archive contains `vidcue-beta.jks`, `credentials.json`, the public certificate
and recovery notes. Preserve both it and the SSH identity. Losing access to the
signing key prevents further in-place updates. No key, password or SSH private
key belongs in chat, Git, a public release asset or an Actions artifact. This local
signing flow needs no GitHub signing secrets.

## Version ordering

`python3 tools/android/release_channel.py code 0.3.0-beta.1` yields `30001`.
The formula is `MAJOR*1000000 + MINOR*10000 + PATCH*100 + stage`, with stage N for
beta.N, 50+N for rc.N and 99 for final. Beta numbers are 1–49; RC numbers 1–48.
Thus beta.2 is 30002, rc.1 is 30051, 0.3.0 is 30099 and 0.3.1-beta.1 is 30101.

## Publication and app updates

`sign-local.py` validates the recovered certificate, owner-only permissions and
clean source, then builds both Rust ABIs and a signed APK, runs release unit/lint
checks and inspects the result. An explicit version code is required; no release
falls back to a debug key.
On disk-constrained build machines, `YTUI_ANDROID_PRUNE_TARGETS=1` removes each
cross-compilation cache after its stripped JNI library has been copied for packaging.

**Publish locally signed Android APK** requires green Android, Rust and Secret
scan push runs for exact current main. It takes only the finished APK and verifies
source, checksum, package, version, certificate, ABIs and alignment before
publishing without rebuilding or re-signing. See [beta publication](BETA1_RELEASE.md)
for manual and temporary metadata-branch handoffs.

Release notes carry a `vidcue-android-update` marker with package, code/name, APK
filename/hash, certificate and minimum SDK. Signed apps check at most every six
hours and on demand in About. The app verifies the download, then Android asks
for installation confirmation. A real beta.1 → beta.2 update/data-retention check
remains required on a device.

The debug Preview publisher refuses beta/RC/final versions even before the first
beta exists, and stops all publication after a marked beta. Later public releases
use only the permanent beta identity.

## Historical alphas

The previous `dev.ytui.android.stable` alpha certificate was
`b3e9f1a70dd6597884d1dbba09eafad3d4d73ea62c4074a9b8cec1ac1e034848`.
Its recovery archive and published alphas remain separate. The
[historical signing record](https://github.com/Vanir20342222/ytui/blob/fc906303c5234bf8b423f4e406df71c5a7157e7d/docs/android/SIGNING.md)
documents it; it is not required for the new beta channel.
