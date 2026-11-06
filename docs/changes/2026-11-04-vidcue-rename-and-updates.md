# Rename to Vidcue, update popup and new README (M9)

- **Date:** 2026-11-04
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** desktop, shared engine, Android (labels and native library name), CI/release scripts, docs
- **Status:** fixture/unit verified (desktop and shared engine); Android not compiled

The user asked for this work, so it touches shared docs, workflows and many files at once
(AGENT_PR_RULES §2/§1.3). The task prompt takes precedence over those rules here.

## What it does
- **Rename:** the app is now **Vidcue** (`vidcue` command, `vidcue-*` crates, `vidcue-dark` and
  `vidcue-light` themes, Android label "Vidcue", `libvidcue_mobile.so`). Old `ytui` folders under
  config, state and cache move once to `vidcue` and leave a symlink behind. Linked folders get a
  link instead of being moved. Existing `~/Videos/ytui` and `~/Music/ytui` folders keep receiving
  downloads. Saved `ytui-dark`/`ytui-light` theme choices load as the new names. `ytui` and
  `ytui-rs` remain installed as aliases.
  - **Unchanged on purpose:** the `ytui-rust` keyring entry, `YTUI_*` variables and secrets, the
    repository URL and existing release asset names, the Android package `dev.ytui.android`
    (and its JNI symbols, notification channels and keystore alias), and dated evidence documents.
- **Update popup:** at startup (online, not demo) the app reads the public GitHub release list.
  A newer desktop release (`v1.2.3` or `desktop-v1.2.3`; Android tags ignored; pre-releases only
  for pre-release builds) opens a popup with **Update now**, **Later** and **Skip this version**.
  - **Update now** suspends the interface, clones that tag and runs its `install.sh`, then returns.
  - The check can be turned off in General settings, run on demand from F2, or run with
    `vidcue update [--check]`.
- **README:** a shorter README with an animated SVG banner, a theme-tour GIF, a player-styles GIF,
  a screenshot grid and the Android preview. The detailed notes moved into collapsible sections.
  All desktop screenshots were re-captured so they show the new name.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Format | `cargo fmt --all -- --check` | pass | unit |
| Lint | `cargo clippy --workspace --all-targets --locked -- -D warnings` | pass | unit |
| Tests | `cargo test --workspace --locked` | pass (all suites, 0 failed) | unit/fixture |
| Folder migration | `paths::rename_tests` (move once, symlink back, linked folders, existing ytui download folder) | pass | unit |
| Installer aliases and rollback | `apps/desktop/tests/install.rs` (vidcue, ytui, ytui-rs; snapshot of old `ytui` binary) | pass | fixture |
| Release selection and versions | `update::tests` (semver order, desktop-only tags, drafts and pre-releases) | pass | unit |
| HTTP check | `update::tests::check_reads_the_release_list_from_the_api` (loopback fixture) | pass | fixture |
| Updater script | `update::tests::updater_clones_the_tag_runs_its_installer_and_cleans_up` (fake git and cargo) | pass | fixture |
| Popup keys, mouse, skip, settings toggle | `apps/desktop/tests/update_popup.rs` | pass | fixture |
| Android tool scripts | `python3 -I tools/android/test_*.py`, `tools/ci/test-should-run.sh`, `tools/ci/test_check_pr.py` | pass | unit |
| Screenshots | `capture_ui` (all modes) and `capture_update` with the demo catalog | regenerated | render |

## Not verified and limits
- Android was not compiled: no SDK in this environment. The Android changes are string and label
  edits, the theme IDs and the native library name (`vidcue_mobile` in `NativeBridge`, the manifest
  and the build scripts).
- No desktop release exists yet: every GitHub release is an `android-*` tag. The popup only appears
  after the owner publishes a `v…` or `desktop-v…` tag newer than the running version.
- "Update now" has not been run against a real GitHub tag. It needs git and cargo, and it builds
  from source (minutes).
- The Android gallery images predate the rename, and some still show "ytui". The README Android
  image uses frames without the old wordmark.
- The GitHub repository is still named `ytui`; renaming it is an owner action. GitHub redirects
  the old URLs, including the API used by the update check.

## For the plan and parity tables
- M9 / desktop: "App renamed to Vidcue (2026-11-04). Old folders migrate automatically, and the
  `ytui`/`ytui-rs` aliases remain."
- Desktop parity: "Release notification: startup GitHub check with Update now / Later / Skip, an
  opt-out setting, an F2 action and `vidcue update [--check]`. Fixture verified. Not yet exercised
  against a published desktop release."
