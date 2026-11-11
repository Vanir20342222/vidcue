package dev.ytui.android

import android.content.Context
import android.content.SharedPreferences
import java.io.File
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import org.json.JSONObject

/** Where a title stopped, so the page can offer "Resume" and a progress line. */
internal data class MovieResume(val positionMs: Long, val durationMs: Long, val updatedAt: Long) {
    val fraction: Float
        get() = if (durationMs <= 0) 0f else (positionMs.toFloat() / durationMs).coerceIn(0f, 1f)
}

/** A finished offline copy in app storage. */
internal data class MovieDownload(
    val key: String,
    val title: String,
    val entrypoint: String,
    val kind: String,
    val bytes: Long,
    val subtitles: Int,
    val source: String,
    val savedAt: Long,
)

/** A download that is running, queued, or failed (kept until dismissed or retried). */
internal data class MovieTransfer(
    val key: String,
    val title: String,
    val bytes: Long = 0,
    val queued: Boolean = true,
    val source: String = "",
    val error: String = "",
)

/**
 * Resume positions and offline copies for Movies (alpha). Downloads run one at a time through the
 * engine's `movie_download` (the shared Cinecat transfer code) into app-private storage, trying
 * each source in order. While one runs, its staging folder's size is the progress shown.
 */
internal object MovieLibrary {
    private const val PREFS = "movies"
    private const val RESUME = "resume"
    private const val DOWNLOADS = "downloads"
    private const val NOTICE = "notice_seen"
    /** Shorter than this, or this close to the end, starts from the beginning. */
    private const val RESUME_MIN_MS = 10_000L
    private const val RESUME_END_MS = 30_000L

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val queue = Mutex()
    private val jobs = mutableMapOf<String, Job>()
    private var prefs: SharedPreferences? = null
    private var root: File? = null

    /** Runs app-lifetime work (a retry download) that must outlive the row that started it. */
    fun launch(block: suspend CoroutineScope.() -> Unit) {
        scope.launch(block = block)
    }

    val resume = MutableStateFlow<Map<String, MovieResume>>(emptyMap())
    val downloads = MutableStateFlow<Map<String, MovieDownload>>(emptyMap())
    val transfers = MutableStateFlow<Map<String, MovieTransfer>>(emptyMap())
    val noticeSeen = MutableStateFlow(true)

    fun init(context: Context) {
        if (prefs != null) return
        val app = context.applicationContext
        val store = app.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        prefs = store
        root = File(app.filesDir, "movies")
        runCatching {
            val metadata = JSONObject(store.getString("catalog_metadata", "{}") ?: "{}")
            metadata.keys().forEach { key -> runCatching { MovieCatalog.rememberMetadata(metadata.getJSONObject(key)) } }
        }
        noticeSeen.value = store.getBoolean(NOTICE, false)
        resume.value = parseResume(store.getString(RESUME, null))
        // Drop records whose folder was removed outside the app.
        downloads.value =
            parseDownloads(store.getString(DOWNLOADS, null)).filterValues { File(it.entrypoint).exists() }
        // A process death mid-download leaves a private staging folder behind.
        scope.launch { root?.listFiles()?.filter { it.name.startsWith(".vidcue-cinecat-") }?.forEach { it.deleteRecursively() } }
    }

    fun rememberTitle(meta: JSONObject): MovieTitle {
        val title = MovieCatalog.rememberMetadata(meta)
        val metadata = runCatching { JSONObject(prefs?.getString("catalog_metadata", "{}") ?: "{}") }.getOrDefault(JSONObject())
        metadata.put(title.id, meta)
        prefs?.edit()?.putString("catalog_metadata", metadata.toString())?.apply()
        return title
    }

    fun acceptNotice() {
        noticeSeen.value = true
        prefs?.edit()?.putBoolean(NOTICE, true)?.apply()
    }

    // Resume -----------------------------------------------------------------------------------

    /** Where to start [key]: a saved position, or 0 near the start or end. */
    fun startPosition(key: String): Long {
        val saved = resume.value[key] ?: return 0
        return if (shouldResume(saved)) saved.positionMs else 0
    }

    fun shouldResume(saved: MovieResume): Boolean =
        saved.positionMs >= RESUME_MIN_MS &&
            (saved.durationMs <= 0 || saved.positionMs < saved.durationMs - RESUME_END_MS)

    fun savePosition(key: String, positionMs: Long, durationMs: Long) {
        val entry = MovieResume(positionMs.coerceAtLeast(0), durationMs.coerceAtLeast(0), System.currentTimeMillis())
        resume.update { if (shouldResume(entry)) it + (key to entry) else it - key }
        persistResume()
    }

    fun clearPosition(key: String) {
        resume.update { it - key }
        persistResume()
    }

    private fun persistResume() {
        val json = JSONObject()
        for ((key, r) in resume.value)
            json.put(key, JSONObject().put("position", r.positionMs).put("duration", r.durationMs).put("at", r.updatedAt))
        prefs?.edit()?.putString(RESUME, json.toString())?.apply()
    }

    // Downloads --------------------------------------------------------------------------------

    /** Queues a download of [pick] from [sources] (tried in order). No-op if already queued or saved. */
    fun download(pick: MoviePick, sources: List<MovieSource>, maxHeight: Int = 1080) =
        download(pick.key, pick.displayTitle, sources, maxHeight)

    fun download(key: String, title: String, sources: List<MovieSource>, maxHeight: Int = 1080) {
        val folder = root ?: return
        if (sources.isEmpty() || key in downloads.value) return
        synchronized(jobs) {
            if (jobs[key]?.isActive == true) return
            transfers.update { it + (key to MovieTransfer(key, title)) }
            jobs[key] =
                scope.launch {
                    try {
                        queue.withLock { run(folder, key, title, sources, maxHeight) }
                    } finally {
                        synchronized(jobs) { jobs.remove(key) }
                    }
                }
        }
    }

    private suspend fun run(folder: File, key: String, title: String, sources: List<MovieSource>, maxHeight: Int) {
        folder.mkdirs()
        val errors = mutableListOf<String>()
        for (source in sources) {
            transfers.update { it + (key to MovieTransfer(key, title, 0, queued = false, source = source.label)) }
            val output = File(folder, "${safeName(key)}-${System.currentTimeMillis()}")
            val meter = scope.launch { meter(folder, key) }
            try {
                val data =
                    RustRpc.call(
                        JSONObject()
                            .put("op", "movie_download")
                            .put("source", source.json())
                            .put("output", output.absolutePath)
                            .put("max_height", maxHeight)
                    ) as? JSONObject ?: throw IllegalStateException("Empty download receipt")
                val record =
                    MovieDownload(
                        key,
                        title,
                        data.optString("entrypoint"),
                        data.optString("kind", source.kind),
                        data.optLong("bytes"),
                        data.optInt("subtitles"),
                        source.label,
                        System.currentTimeMillis(),
                    )
                downloads.update { it + (key to record) }
                persistDownloads()
                transfers.update { it - key }
                return
            } catch (cancelled: CancellationException) {
                transfers.update { it - key }
                throw cancelled
            } catch (error: Exception) {
                errors += "${source.label}: ${error.message ?: "failed"}"
                output.deleteRecursively()
            } finally {
                meter.cancel()
            }
        }
        transfers.update { it + (key to MovieTransfer(key, title, queued = false, error = errors.joinToString("\n"))) }
    }

    /** Reports the staging folder's size as progress; only one download runs at a time. */
    private suspend fun meter(folder: File, key: String) {
        while (scope.isActive) {
            val staging = folder.listFiles()?.filter { it.name.startsWith(".vidcue-cinecat-") }.orEmpty()
            val bytes = staging.sumOf { dir -> dir.walkTopDown().filter { it.isFile }.sumOf { it.length() } }
            transfers.update { map -> map[key]?.let { map + (key to it.copy(bytes = bytes)) } ?: map }
            delay(700)
        }
    }

    fun cancel(key: String) {
        synchronized(jobs) { jobs.remove(key) }?.cancel()
        transfers.update { it - key }
    }

    fun dismiss(key: String) = transfers.update { it - key }

    fun delete(key: String) {
        val record = downloads.value[key] ?: return
        downloads.update { it - key }
        persistDownloads()
        val base = root ?: return
        scope.launch {
            // The entrypoint sits in the bundle folder (HLS index.m3u8 or video.media).
            val bundle = File(record.entrypoint).parentFile
            if (bundle != null && bundle.parentFile?.canonicalPath == base.canonicalPath) bundle.deleteRecursively()
        }
    }

    private fun persistDownloads() {
        val json = JSONObject()
        for ((key, d) in downloads.value)
            json.put(
                key,
                JSONObject()
                    .put("title", d.title)
                    .put("entrypoint", d.entrypoint)
                    .put("kind", d.kind)
                    .put("bytes", d.bytes)
                    .put("subtitles", d.subtitles)
                    .put("source", d.source)
                    .put("at", d.savedAt),
            )
        prefs?.edit()?.putString(DOWNLOADS, json.toString())?.apply()
    }

    internal fun safeName(key: String): String = key.replace(Regex("[^A-Za-z0-9._-]"), "_").take(80)

    internal fun parseResume(raw: String?): Map<String, MovieResume> {
        val json = runCatching { JSONObject(raw ?: return emptyMap()) }.getOrNull() ?: return emptyMap()
        return json.keys().asSequence().mapNotNull { key ->
            val r = json.optJSONObject(key) ?: return@mapNotNull null
            key to MovieResume(r.optLong("position"), r.optLong("duration"), r.optLong("at"))
        }.toMap()
    }

    internal fun parseDownloads(raw: String?): Map<String, MovieDownload> {
        val json = runCatching { JSONObject(raw ?: return emptyMap()) }.getOrNull() ?: return emptyMap()
        return json.keys().asSequence().mapNotNull { key ->
            val d = json.optJSONObject(key) ?: return@mapNotNull null
            val entry = d.optString("entrypoint").ifBlank { return@mapNotNull null }
            key to
                MovieDownload(
                    key,
                    d.optString("title"),
                    entry,
                    d.optString("kind"),
                    d.optLong("bytes"),
                    d.optInt("subtitles"),
                    d.optString("source"),
                    d.optLong("at"),
                )
        }.toMap()
    }
}
