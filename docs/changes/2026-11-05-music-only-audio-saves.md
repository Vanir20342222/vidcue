# M9: scope Android audio-only saves to Music

The native Android save path now applies `download.audio` only to Music songs,
Music URLs and collections opened as Music. Regular YouTube videos and video
collections retain video with either switch position. Music saves request audio
when enabled and video when disabled; normal song/radio playback is unchanged.
The selected mode is captured before resolution and is passed through the existing
single-item and playlist preparation paths. Existing downloads are preserved.
The setting description now states its YouTube Music scope.

Added three JUnit regression tests covering both preference values for ordinary
videos/collections, Music songs/URLs/collections and radio.

Validation: `git diff --check` passed. The focused Gradle unit-test command could not start:
the uncached Gradle distribution download failed with
`UnknownHostException: services.gradle.org`. A standalone Java harness attempt
was also blocked because `javac` is not installed (only a Java runtime is present).
No Android SDK is configured here;
full Kotlin/Android compilation, JUnit execution, APK build and device/live-service
validation remain unverified. No release was published.
