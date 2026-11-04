package dev.ytui.android

import java.io.File
import java.security.MessageDigest
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okio.Buffer
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

/** Offline: synthetic releases on a loopback server; no GitHub, key or device involved. */
@RunWith(RobolectricTestRunner::class)
class AppUpdatesTest {
    private val cert = "b".repeat(64)
    private val pkg = "dev.vidcue.android.stable"
    private lateinit var server: MockWebServer
    private val defaults = AppUpdates.releasesUrl to AppUpdates.downloadPrefix

    @Before
    fun start() {
        server = MockWebServer().apply { start() }
        AppUpdates.releasesUrl = server.url("/releases").toString()
        AppUpdates.downloadPrefix = server.url("/download/").toString()
        AppUpdates.dismiss()
    }

    @After
    fun stop() {
        AppUpdates.releasesUrl = defaults.first
        AppUpdates.downloadPrefix = defaults.second
        AppUpdates.dismiss()
        server.shutdown()
    }

    private fun release(
        tag: String,
        code: Long,
        packageName: String = pkg,
        certSha: String = cert,
        sha: String = "a".repeat(64),
        minSdk: Int = 26,
        draft: Boolean = false,
        marker: Boolean = true,
        url: String = server.url("/download/$tag/vidcue-$code.apk").toString(),
    ): JSONObject {
        val data =
            JSONObject()
                .put("package", packageName)
                .put("versionCode", code)
                .put("versionName", tag.removePrefix("android-"))
                .put("apk", "vidcue-$code.apk")
                .put("sha256", sha)
                .put("certSha256", certSha)
                .put("minSdk", minSdk)
        val body = "Notes **here**.\n\n" + if (marker) "<!-- vidcue-android-update $data -->\n" else ""
        return JSONObject()
            .put("tag_name", tag)
            .put("draft", draft)
            .put("html_url", "https://github.com/Vanir20342222/ytui/releases/tag/$tag")
            .put("body", body)
            .put(
                "assets",
                JSONArray().put(
                    JSONObject()
                        .put("name", "vidcue-$code.apk")
                        .put("size", 4)
                        .put("browser_download_url", url)
                ),
            )
    }

    private fun installed(code: Long = 300100, certs: Set<String> = setOf(cert), sdk: Int = 34) =
        InstalledApp(pkg, code, certs, sdk)

    @Test
    fun parseKeepsOnlyMarkedStableReleasesWithTheirApk() {
        val list =
            JSONArray()
                .put(release("android-0.3.0-beta.2", 300200))
                .put(release("android-preview-0.3.0-beta.9", 300900))
                .put(release("android-0.3.0-beta.8", 300800, draft = true))
                .put(release("android-0.2.0-alpha.12-mobile.10", 10, marker = false))
                .put(release("android-0.3.0-beta.7", 300700, url = "https://example.invalid/x.apk"))
                .put(release("android-0.3.0-beta.6", 300600, sha = "nothex"))
        val parsed = AppUpdates.parse(list.toString())
        assertEquals(listOf(300200L), parsed.map { it.versionCode })
        val only = parsed.single()
        assertEquals("0.3.0-beta.2", only.versionName)
        assertEquals(cert, only.certSha256)
        assertEquals(4L, only.apkSize)
    }

    @Test
    fun newestRequiresSamePackageSameKeyHigherCodeAndSupportedAndroid() {
        val releases =
            AppUpdates.parse(
                JSONArray()
                    .put(release("android-a", 300200))
                    .put(release("android-b", 300300, certSha = "c".repeat(64)))
                    .put(release("android-c", 300400, packageName = "dev.vidcue.android.preview"))
                    .put(release("android-d", 300500, minSdk = 35))
                    .put(release("android-e", 300050))
                    .toString()
            )
        assertEquals(300200L, AppUpdates.newest(releases, installed())?.versionCode)
        assertEquals(300500L, AppUpdates.newest(releases, installed(sdk = 35))?.versionCode)
        assertNull(AppUpdates.newest(releases, installed(code = 300500, sdk = 35)))
        assertNull(AppUpdates.newest(releases, installed(certs = emptySet())))
    }

    private fun serve(vararg releases: JSONObject) {
        server.enqueue(MockResponse().setBody(JSONArray(releases.toList()).toString()))
    }

    @Test
    fun manualCheckReportsAvailableOrUpToDateAndSkipOnlyQuietsAutomaticChecks() = runBlocking {
        val context = RuntimeEnvironment.getApplication()
        serve(release("android-0.3.0-beta.2", 300200))
        AppUpdates.runCheck(context, manual = true, app = installed())
        val available = AppUpdates.state.value as UpdateState.Available
        assertEquals(300200L, available.release.versionCode)

        AppUpdates.skip(context, available.release)
        assertEquals(UpdateState.Idle, AppUpdates.state.value)
        serve(release("android-0.3.0-beta.2", 300200))
        AppUpdates.runCheck(context, manual = false, app = installed())
        assertEquals(UpdateState.Idle, AppUpdates.state.value)
        serve(release("android-0.3.0-beta.2", 300200))
        AppUpdates.runCheck(context, manual = true, app = installed())
        assertTrue(AppUpdates.state.value is UpdateState.Available)

        serve(release("android-0.3.0-beta.2", 300200))
        AppUpdates.runCheck(context, manual = true, app = installed(code = 300200))
        assertTrue((AppUpdates.state.value as UpdateState.Note).text.contains("up to date"))

        server.enqueue(MockResponse().setResponseCode(503))
        AppUpdates.runCheck(context, manual = true, app = installed())
        assertTrue((AppUpdates.state.value as UpdateState.Note).text.contains("Could not reach GitHub"))
        server.enqueue(MockResponse().setResponseCode(503))
        AppUpdates.runCheck(context, manual = false, app = installed())
        assertEquals(UpdateState.Idle, AppUpdates.state.value)

        server.enqueue(MockResponse().setBody("not a release list"))
        AppUpdates.runCheck(context, manual = true, app = installed())
        assertTrue((AppUpdates.state.value as UpdateState.Note).text.contains("Could not reach GitHub"))
    }

    @Test
    fun automaticCheckSettingIsOnByDefaultAndRemembered() {
        val context = RuntimeEnvironment.getApplication()
        assertTrue(AppUpdates.enabled(context))
        AppUpdates.setEnabled(context, false)
        assertFalse(AppUpdates.enabled(context))
        AppUpdates.setEnabled(context, true)
    }

    private fun sha(bytes: ByteArray) =
        MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }

    @Test
    fun downloadKeepsOnlyAnApkThatMatchesItsPublishedChecksum() = runBlocking {
        val context = RuntimeEnvironment.getApplication()
        val payload = "fake apk bytes".toByteArray()
        val good =
            AppUpdates.parse(
                    JSONArray().put(release("android-x", 300200, sha = sha(payload))).toString()
                )
                .single()
        server.enqueue(MockResponse().setBody(Buffer().write(payload)))
        val progress = mutableListOf<Float>()
        val file = AppUpdates.download(context, good) { progress += it }
        assertTrue(file.readBytes().contentEquals(payload))
        assertEquals(1f, progress.last())

        val bad = good.copy(sha256 = "0".repeat(64))
        server.enqueue(MockResponse().setBody(Buffer().write(payload)))
        val error = runCatching { AppUpdates.download(context, bad) {} }.exceptionOrNull()
        assertTrue(error?.message.orEmpty().contains("checksum"))
        assertFalse(File(context.cacheDir, "updates/vidcue-300200.apk").exists())

        val elsewhere = good.copy(apkUrl = "https://example.invalid/vidcue.apk")
        assertTrue(runCatching { AppUpdates.download(context, elsewhere) {} }.isFailure)
    }

    @Test
    fun aDownloadThatIsNotAnApkIsRejectedBeforeTheInstaller() {
        val context = RuntimeEnvironment.getApplication()
        val file = File(context.cacheDir, "not-an.apk").apply { writeText("plain text") }
        val release =
            AppUpdates.parse(JSONArray().put(release("android-x", 300200)).toString()).single()
        assertEquals("The download is not a valid Android app.", AppUpdates.verifyArchive(context, file, release))
    }

    @Test
    fun cancellingAnUpdateStopsProgressAndRemovesThePartialApk() = runBlocking {
        val context = RuntimeEnvironment.getApplication()
        val payload = ByteArray(256 * 1024) { 7 }
        val release = AppUpdates.parse(
            JSONArray().put(release("android-cancel", 30002, sha = sha(payload))).toString()
        ).single()
        server.enqueue(MockResponse().setBody(Buffer().write(payload)))
        val downloadJob = Job()
        var reports = 0
        val error = runCatching {
            withContext(downloadJob) {
                AppUpdates.download(context, release) {
                    reports++
                    downloadJob.cancel()
                }
            }
        }.exceptionOrNull()
        assertTrue(error is CancellationException)
        assertEquals(1, reports)
        assertFalse(File(context.cacheDir, "updates/vidcue-30002.apk").exists())
    }
}
