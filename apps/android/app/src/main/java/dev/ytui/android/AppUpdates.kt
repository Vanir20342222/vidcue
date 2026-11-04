package dev.ytui.android

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInfo
import android.content.pm.PackageInstaller
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.Settings
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject

/**
 * One owner-signed release that can replace this installation in place. Built only from
 * releases whose notes carry the `vidcue-android-update` marker written by
 * `tools/android/verify-local-release.py` (see `tools/android/release_channel.py`).
 */
internal data class AppRelease(
    val tag: String,
    val versionName: String,
    val versionCode: Long,
    val packageName: String,
    val apkUrl: String,
    val apkSize: Long,
    val sha256: String,
    val certSha256: String,
    val minSdk: Int,
    val page: String,
)

internal sealed interface UpdateState {
    data object Idle : UpdateState

    data object Checking : UpdateState

    /** Result of a check the user started: up to date, or why nothing can be offered. */
    data class Note(val text: String) : UpdateState

    data class Available(val release: AppRelease) : UpdateState

    data class Downloading(val release: AppRelease, val progress: Float) : UpdateState

    /** Android's own confirmation screen is open (or about to open). */
    data class Installing(val release: AppRelease) : UpdateState

    /** "Install unknown apps" is off for Vidcue; the user turns it on once. */
    data class NeedsPermission(val release: AppRelease) : UpdateState

    data class Failed(val text: String, val release: AppRelease?) : UpdateState
}

/** What this installation is, for choosing a release it can be replaced with. */
internal data class InstalledApp(
    val packageName: String,
    val versionCode: Long,
    val certs: Set<String>,
    val sdk: Int,
)

/**
 * GitHub release check, download, verification and in-place install through Android's
 * PackageInstaller. Android itself still refuses any APK with another package name, another
 * signing key or a lower version code; the checks here only explain problems up front.
 */
internal object AppUpdates {
    const val REPOSITORY = "Vanir20342222/ytui"
    private const val MARKER = "vidcue-android-update"
    private const val PREFS = "vidcue-updates"
    private const val AUTO_INTERVAL_MS = 6L * 60 * 60 * 1000
    private const val MAX_LIST_BYTES = 4L * 1024 * 1024
    const val MAX_APK_BYTES = 300L * 1024 * 1024
    private val markerPattern =
        Regex("<!--\\s*$MARKER\\s+(\\{.*?\\})\\s*-->", RegexOption.DOT_MATCHES_ALL)
    private val hex64 = Regex("[0-9a-f]{64}")

    /** Overridable for loopback tests only. */
    internal var releasesUrl = "https://api.github.com/repos/$REPOSITORY/releases?per_page=30"
    internal var downloadPrefix = "https://github.com/$REPOSITORY/releases/download/"

    private val mutable = MutableStateFlow<UpdateState>(UpdateState.Idle)
    val state: StateFlow<UpdateState> = mutable.asStateFlow()
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var job: Job? = null

    // ---- Release list ----

    /** Marker-carrying, non-draft, non-Preview releases with a matching APK asset. */
    fun parse(json: String): List<AppRelease> {
        val list = JSONArray(json)
        val releases = mutableListOf<AppRelease>()
        for (i in 0 until list.length()) {
            val release = list.optJSONObject(i) ?: continue
            val tag = release.optString("tag_name")
            if (release.optBoolean("draft") || !tag.startsWith("android-") ||
                tag.startsWith("android-preview-"))
                continue
            val marker = markerPattern.find(release.optString("body")) ?: continue
            val data = runCatching { JSONObject(marker.groupValues[1]) }.getOrNull() ?: continue
            val apkName = data.optString("apk")
            val assets = release.optJSONArray("assets") ?: JSONArray()
            val asset =
                (0 until assets.length())
                    .mapNotNull { assets.optJSONObject(it) }
                    .firstOrNull { it.optString("name") == apkName && apkName.endsWith(".apk") }
                    ?: continue
            val url = asset.optString("browser_download_url")
            val sha256 = data.optString("sha256").lowercase()
            val cert = data.optString("certSha256").lowercase()
            val code = data.optLong("versionCode")
            if (!url.startsWith(downloadPrefix) || !hex64.matches(sha256) ||
                !hex64.matches(cert) || code <= 0)
                continue
            releases +=
                AppRelease(
                    tag = tag,
                    versionName = data.optString("versionName").ifBlank { tag.removePrefix("android-") },
                    versionCode = code,
                    packageName = data.optString("package"),
                    apkUrl = url,
                    apkSize = asset.optLong("size"),
                    sha256 = sha256,
                    certSha256 = cert,
                    minSdk = data.optInt("minSdk", 26),
                    page = release.optString("html_url"),
                )
        }
        return releases
    }

    /** Newest release that Android will accept as an in-place update of [app]. */
    fun newest(releases: List<AppRelease>, app: InstalledApp): AppRelease? =
        releases
            .filter {
                it.packageName == app.packageName &&
                    it.versionCode > app.versionCode &&
                    it.minSdk <= app.sdk &&
                    it.certSha256 in app.certs
            }
            .maxByOrNull { it.versionCode }

    // ---- Settings ----

    private fun prefs(context: Context) =
        context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    fun enabled(context: Context) = prefs(context).getBoolean("enabled", true)

    fun setEnabled(context: Context, enabled: Boolean) =
        prefs(context).edit().putBoolean("enabled", enabled).apply()

    fun skip(context: Context, release: AppRelease) {
        prefs(context).edit().putLong("skipped", release.versionCode).apply()
        dismiss()
    }

    /** "Later": hide the popup and stop a running download. */
    fun dismiss() {
        job?.cancel()
        job = null
        mutable.value = UpdateState.Idle
    }

    // ---- Check ----

    /** Background check when the app opens; quiet, at most every few hours. */
    fun checkOnStart(context: Context) {
        val app = context.applicationContext
        if (job?.isActive == true || mutable.value is UpdateState.Installing) return
        File(app.cacheDir, "updates").deleteRecursively()
        if (BuildConfig.DEBUG || MediaBridge.offlineUi || !enabled(app)) return
        if (System.currentTimeMillis() - prefs(app).getLong("checked", 0) < AUTO_INTERVAL_MS) return
        launch { runCheck(app, manual = false) }
    }

    /** About → Check for updates. Always asks GitHub and reports the outcome. */
    fun checkNow(context: Context) = launch { runCheck(context.applicationContext, manual = true) }

    private fun launch(block: suspend () -> Unit) {
        if (job?.isActive == true) return
        job = scope.launch { block() }
    }

    internal suspend fun runCheck(
        context: Context,
        manual: Boolean,
        app: InstalledApp = installed(context),
    ) {
        mutable.value = UpdateState.Checking
        val releases =
            runCatching { withContext(Dispatchers.IO) { parse(fetchText(releasesUrl)) } }
                .getOrElse {
                    if (it is kotlinx.coroutines.CancellationException) throw it
                    mutable.value =
                        if (manual) UpdateState.Note("Could not reach GitHub. Try again later.")
                        else UpdateState.Idle
                    return
                }
        prefs(context).edit().putLong("checked", System.currentTimeMillis()).apply()
        val release = newest(releases, app)
        mutable.value =
            when {
                release == null && manual && app.packageName.endsWith(".preview") ->
                    UpdateState.Note(
                        "Preview builds are not updated in place. Install the signed Vidcue app to get updates."
                    )
                release == null ->
                    if (manual) UpdateState.Note("Vidcue ${BuildConfig.VERSION_NAME} is up to date.")
                    else UpdateState.Idle
                !manual && prefs(context).getLong("skipped", 0) >= release.versionCode ->
                    UpdateState.Idle
                else -> UpdateState.Available(release)
            }
    }

    // ---- Install ----

    /** "Update now": download, verify and hand the APK to Android's installer. */
    fun start(context: Context, release: AppRelease) {
        val app = context.applicationContext
        if (!app.packageManager.canRequestPackageInstalls()) {
            mutable.value = UpdateState.NeedsPermission(release)
            return
        }
        job?.cancel()
        job = scope.launch { install(app, release) }
    }

    /** Opens Android's "Install unknown apps" switch for Vidcue. */
    fun openInstallPermission(context: Context) {
        val intent =
            Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${context.packageName}"))
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        runCatching { context.startActivity(intent) }
    }

    private suspend fun install(context: Context, release: AppRelease) {
        mutable.value = UpdateState.Downloading(release, 0f)
        val file =
            runCatching {
                    withContext(Dispatchers.IO) {
                        download(context, release) { progress ->
                            mutable.value = UpdateState.Downloading(release, progress)
                        }
                    }
                }
                .getOrElse { error ->
                    if (error is kotlinx.coroutines.CancellationException) throw error
                    mutable.value = UpdateState.Failed(error.message ?: "Download failed.", release)
                    return
                }
        verifyArchive(context, file, release)?.let { problem ->
            file.delete()
            mutable.value = UpdateState.Failed(problem, release)
            return
        }
        mutable.value = UpdateState.Installing(release)
        runCatching { withContext(Dispatchers.IO) { commit(context, file) } }
            .onFailure {
                mutable.value =
                    UpdateState.Failed("Android could not start the installation.", release)
            }
    }

    /** Called by [AppUpdateReceiver] with Android's installer result. */
    internal fun installFinished(message: String?) {
        val release = (mutable.value as? UpdateState.Installing)?.release
        mutable.value =
            if (message == null) UpdateState.Idle else UpdateState.Failed(message, release)
    }

    internal suspend fun download(context: Context, release: AppRelease, progress: (Float) -> Unit): File {
        val coroutine = currentCoroutineContext()
        require(release.apkUrl.startsWith(downloadPrefix)) { "Unexpected download address." }
        val dir = File(context.cacheDir, "updates").apply { mkdirs() }
        val file = File(dir, "vidcue-${release.versionCode}.apk")
        file.delete()
        try {
            val connection = open(release.apkUrl)
            try {
                check(connection.responseCode == 200) {
                    "Download failed (HTTP ${connection.responseCode})."
                }
                val total = connection.contentLengthLong.takeIf { it > 0 } ?: release.apkSize
                check(total <= MAX_APK_BYTES) { "The update is unexpectedly large." }
                val digest = MessageDigest.getInstance("SHA-256")
                var read = 0L
                var reported = -1
                connection.inputStream.use { input ->
                    file.outputStream().use { output ->
                        val buffer = ByteArray(64 * 1024)
                        while (true) {
                            coroutine.ensureActive()
                            val count = input.read(buffer)
                            if (count < 0) break
                            coroutine.ensureActive()
                            read += count
                            check(read <= MAX_APK_BYTES) { "The update is unexpectedly large." }
                            digest.update(buffer, 0, count)
                            output.write(buffer, 0, count)
                            if (total > 0) {
                                val percent = (read * 100 / total).toInt().coerceIn(0, 100)
                                if (percent != reported) {
                                    reported = percent
                                    progress(percent / 100f)
                                }
                            }
                        }
                    }
                }
                check(hex(digest.digest()) == release.sha256) {
                    "The download did not match its published checksum."
                }
            } finally {
                connection.disconnect()
            }
            return file
        } catch (error: Throwable) {
            file.delete()
            throw error
        }
    }

    /** Explains a mismatch before Android's installer would reject it. Null when fine. */
    @Suppress("DEPRECATION")
    internal fun verifyArchive(context: Context, file: File, release: AppRelease): String? {
        val flags =
            if (Build.VERSION.SDK_INT >= 28) PackageManager.GET_SIGNING_CERTIFICATES
            else PackageManager.GET_SIGNATURES
        val info =
            runCatching { context.packageManager.getPackageArchiveInfo(file.path, flags) }.getOrNull()
                ?: return "The download is not a valid Android app."
        if (info.packageName != context.packageName) return "The download is for a different app."
        val code = if (Build.VERSION.SDK_INT >= 28) info.longVersionCode else info.versionCode.toLong()
        if (code != release.versionCode || code <= BuildConfig.VERSION_CODE)
            return "The download has an unexpected version."
        val archive = certs(info)
        if (release.certSha256 !in archive || archive.intersect(installed(context).certs).isEmpty())
            return "The download is not signed with Vidcue's key."
        return null
    }

    private fun commit(context: Context, file: File) {
        val installer = context.packageManager.packageInstaller
        val params =
            PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL).apply {
                setAppPackageName(context.packageName)
                setSize(file.length())
            }
        val id = installer.createSession(params)
        try {
            installer.openSession(id).use { session ->
                file.inputStream().use { input ->
                    session.openWrite("vidcue.apk", 0, file.length()).use { output ->
                        input.copyTo(output)
                        session.fsync(output)
                    }
                }
                val result =
                    Intent(context, AppUpdateReceiver::class.java).setPackage(context.packageName)
                val flags =
                    PendingIntent.FLAG_UPDATE_CURRENT or
                        (if (Build.VERSION.SDK_INT >= 31) PendingIntent.FLAG_MUTABLE else 0)
                session.commit(PendingIntent.getBroadcast(context, id, result, flags).intentSender)
            }
        } catch (error: Exception) {
            installer.abandonSession(id)
            throw error
        } finally {
            file.delete()
        }
    }

    // ---- Identity and HTTP ----

    fun installed(context: Context): InstalledApp {
        val certs =
            runCatching {
                    val pm = context.packageManager
                    @Suppress("DEPRECATION")
                    val info =
                        if (Build.VERSION.SDK_INT >= 28)
                            pm.getPackageInfo(context.packageName, PackageManager.GET_SIGNING_CERTIFICATES)
                        else pm.getPackageInfo(context.packageName, PackageManager.GET_SIGNATURES)
                    certs(info)
                }
                .getOrDefault(emptySet())
        return InstalledApp(
            context.packageName,
            BuildConfig.VERSION_CODE.toLong(),
            certs,
            Build.VERSION.SDK_INT,
        )
    }

    @Suppress("DEPRECATION")
    private fun certs(info: PackageInfo): Set<String> {
        val signatures =
            if (Build.VERSION.SDK_INT >= 28) {
                val signing = info.signingInfo ?: return emptySet()
                if (signing.hasMultipleSigners()) signing.apkContentsSigners
                else signing.signingCertificateHistory
            } else info.signatures
        return signatures.orEmpty().map { sha256(it.toByteArray()) }.toSet()
    }

    private fun open(url: String): HttpURLConnection =
        (URL(url).openConnection() as HttpURLConnection).apply {
            connectTimeout = 15_000
            readTimeout = 30_000
            instanceFollowRedirects = true
            setRequestProperty("User-Agent", "Vidcue-Android/${BuildConfig.VERSION_NAME}")
            setRequestProperty("Accept", "application/vnd.github+json")
        }

    private fun fetchText(url: String): String {
        val connection = open(url)
        try {
            check(connection.responseCode == 200) { "HTTP ${connection.responseCode}" }
            val bytes = connection.inputStream.use { it.readNBytesCompat(MAX_LIST_BYTES) }
            return String(bytes, Charsets.UTF_8)
        } finally {
            connection.disconnect()
        }
    }

    private fun java.io.InputStream.readNBytesCompat(limit: Long): ByteArray {
        val output = java.io.ByteArrayOutputStream()
        val buffer = ByteArray(16 * 1024)
        while (true) {
            val count = read(buffer)
            if (count < 0) return output.toByteArray()
            output.write(buffer, 0, count)
            check(output.size() <= limit) { "Release list too large" }
        }
    }

    internal fun sha256(bytes: ByteArray) = hex(MessageDigest.getInstance("SHA-256").digest(bytes))

    private fun hex(bytes: ByteArray) = bytes.joinToString("") { "%02x".format(it) }
}
