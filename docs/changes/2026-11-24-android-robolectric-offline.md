# Robolectric runtimes resolved through Gradle (M9)

- **Date:** 2026-11-24
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android (build/test only)
- **Status:** fixture/unit verified

## What it does
The Android JVM unit tests no longer download Robolectric's Android runtime jars at test time.
Gradle resolves them as ordinary dependencies (one configuration per runtime: SDK 35 and SDK 21),
copies them to `app/build/robolectric-runtime`, and the tests run Robolectric in offline mode
against that directory. Since 2026-11-23 the runtime download inside the `apk` job on `main`
failed with an `IOException` in `MavenArtifactFetcher`, failing 125–156 tests per run and keeping
`main` (and therefore every release) red.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Reproduce | Moved `~/.m2/repository/org/robolectric/android-all-instrumented` away, ran `:app:testDebugUnitTest` with offline mode but only one configuration | same `MavenArtifactFetcher` failures (SDK 21 runtime missing after conflict resolution) | fixture/unit |
| Fix | Same, with one configuration per runtime | `BUILD SUCCESSFUL`, both jars synced, local Maven cache untouched | fixture/unit |

## Not verified and limits
- The root cause of the CI download failure (runner network or Maven Central behaviour) was not identified; this change removes the dependency on it.
- When `org.robolectric:robolectric` is upgraded, the two runtime versions in `app/build.gradle` must be updated with it.

## For the plan and parity tables
- Android JVM tests run Robolectric offline with Gradle-resolved runtimes (2026-11-24).
