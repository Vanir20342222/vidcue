package dev.ytui.android

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject

/**
 * Movies (alpha): a cinematic page over the shared HLS engine. A featured hero, poster rows,
 * search, continue watching, downloads, a "Play a stream URL" panel, and a detail sheet with the
 * season/episode picker and each source's check result. Mirrors the desktop Movies page, but its
 * catalog is openly licensed films and public test streams: no third-party source lookup.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun MoviesPage(vm: YtuiViewModel) {
    val context = LocalContext.current
    // Idempotent: reads the saved state once per process.
    MovieLibrary.init(context)
    val noticeSeen by MovieLibrary.noticeSeen.collectAsState()
    val resume by MovieLibrary.resume.collectAsState()
    val downloads by MovieLibrary.downloads.collectAsState()
    val transfers by MovieLibrary.transfers.collectAsState()
    var query by rememberSaveable { mutableStateOf("") }
    var openId by rememberSaveable { mutableStateOf<String?>(null) }
    var openSeason by rememberSaveable { mutableStateOf<Int?>(null) }
    var openEpisode by rememberSaveable { mutableStateOf<Int?>(null) }
    fun open(pick: MoviePick) {
        openId = pick.title.id
        openSeason = pick.season
        openEpisode = pick.episode
    }
    val films = remember { MovieCatalog.titles.filter { !it.series && !it.cinecat } }
    val series = remember { MovieCatalog.titles.filter { it.series && !it.cinecat } }
    val cinecat = remember { MovieCatalog.titles.filter { it.cinecat } }
    val continuing =
        remember(resume) {
            resume.entries
                .sortedByDescending { it.value.updatedAt }
                .mapNotNull { (key, r) -> MovieCatalog.pick(key)?.let { it to r } }
                .filter { MovieLibrary.shouldResume(it.second) }
        }
    val featured = continuing.firstOrNull()?.first?.title ?: MovieCatalog.titles.first()
    val results = remember(query) { MovieCatalog.search(query) }
    val bottom = LocalBottomOverlay.current

    LazyColumn(
        Modifier.fillMaxSize().testTag("movies-page"),
        contentPadding = PaddingValues(bottom = bottom + 24.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        if (!noticeSeen) item("notice") { AlphaNotice(MovieLibrary::acceptNotice) }
        item("hero") {
            Hero(
                featured,
                resumeOf = continuing.firstOrNull { it.first.title.id == featured.id },
                onPlay = { open(continuing.firstOrNull { it.first.title.id == featured.id }?.first ?: MoviePick(featured)) },
                onDetails = { open(MoviePick(featured)) },
            )
        }
        item("search") {
            MediaTextField(
                query,
                { query = it },
                "Search films, test streams and Cinecat",
                Modifier.fillMaxWidth().padding(horizontal = MediaTokens.textInset).testTag("movies-search"),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            )
        }
        if (query.isNotBlank())
            item("results") {
                if (results.isEmpty())
                    Text(
                        "Nothing matches “${query.trim()}”.",
                        Modifier.padding(horizontal = MediaTokens.textInset),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                else PosterRow("Results", results, resume) { open(MoviePick(it)) }
            }
        if (continuing.isNotEmpty())
            item("continue") {
                Shelf("Continue watching") {
                    items(continuing, key = { it.first.key }) { (pick, r) ->
                        ContinueCard(pick, r) { open(pick) }
                    }
                }
            }
        if (transfers.isNotEmpty() || downloads.isNotEmpty())
            item("downloads") { DownloadsSection(transfers, downloads, onOpen = { key -> MovieCatalog.pick(key)?.let(::open) }) }
        item("films") { PosterRow("Open films", films, resume) { open(MoviePick(it)) } }
        item("series") { PosterRow("Test streams", series, resume) { open(MoviePick(it)) } }
        if (cinecat.isNotEmpty()) item("cinecat") { PosterRow("Cinecat", cinecat, resume) { open(MoviePick(it)) } }
        item("url") { StreamUrlPanel(vm) }
        item("credits") {
            Text(
                "Films are shared by their creators under Creative Commons licences and credited " +
                    "on each title. Stream Lab episodes are public player-test streams. Cinecat " +
                    "titles resolve through the Cinecat sources API, whose providers decide what " +
                    "serves; availability depends on the providers and your network.",
                Modifier.padding(horizontal = MediaTokens.textInset),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }

    val selected = openId?.let(MovieCatalog::find)
    if (selected != null) {
        val state = rememberModalBottomSheetState(skipPartiallyExpanded = true)
        ModalBottomSheet(
            onDismissRequest = { openId = null },
            sheetState = state,
            shape = MediaTokens.sheet,
            containerColor = lerp(MaterialTheme.colorScheme.background, MaterialTheme.colorScheme.surface, 0.6f),
            contentColor = MaterialTheme.colorScheme.onSurface,
            tonalElevation = 0.dp,
            scrimColor = Color.Black.copy(alpha = 0.62f),
        ) {
            MovieSheet(
                vm,
                selected,
                initialSeason = openSeason,
                initialEpisode = openEpisode,
                onDismiss = { openId = null },
            )
        }
    }
}

@Composable
private fun AlphaNotice(accept: () -> Unit) {
    Column(
        Modifier.padding(horizontal = MediaTokens.textInset)
            .fillMaxWidth()
            .clip(MediaTokens.card)
            .background(MaterialTheme.colorScheme.surfaceContainer)
            .padding(16.dp)
            .testTag("movies-notice"),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            MediaIcon(Glyph.Film, Modifier.size(22.dp))
            Text("Movies (alpha)", style = MaterialTheme.typography.titleMedium)
        }
        Text(
            "A test of the shared movie engine: openly licensed films and public test streams, " +
                "with source checks and failover, quality, audio and subtitle menus, resume and " +
                "offline downloads. Playback here has not yet been qualified on every device.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Pill("Got it", Modifier.align(Alignment.End), PillStyle.Tonal, onClick = accept)
    }
}

/** Full-width featured title: artwork with a deep scrim, title, meta and two actions. */
@Composable
private fun Hero(title: MovieTitle, resumeOf: Pair<MoviePick, MovieResume>?, onPlay: () -> Unit, onDetails: () -> Unit) {
    val height = (LocalConfiguration.current.screenHeightDp * 0.48f).dp.coerceIn(280.dp, 460.dp)
    val background = MaterialTheme.colorScheme.background
    Box(Modifier.fillMaxWidth().height(height).testTag("movies-hero")) {
        MoviePoster(title, Modifier.fillMaxSize(), shape = null)
        Box(
            Modifier.fillMaxSize()
                .background(
                    Brush.verticalGradient(
                        0f to background.copy(alpha = 0.1f),
                        0.45f to background.copy(alpha = 0.35f),
                        1f to background,
                    )
                )
        )
        Column(
            Modifier.align(Alignment.BottomStart).padding(horizontal = MediaTokens.textInset).fillMaxWidth(),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                if (resumeOf != null) "Continue watching" else "Featured",
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary,
            )
            Text(title.title, style = MaterialTheme.typography.displaySmall, maxLines = 2, overflow = TextOverflow.Ellipsis)
            Text(movieMeta(title), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Text(title.synopsis, style = MaterialTheme.typography.bodyMedium, maxLines = 2, overflow = TextOverflow.Ellipsis)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(top = 4.dp)) {
                Pill(
                    if (resumeOf != null) "Resume" else if (title.series) "Episodes" else "Play",
                    Modifier.testTag("movies-hero-play"),
                    PillStyle.Primary,
                    glyph = Glyph.Play,
                    onClick = onPlay,
                )
                Pill("Details", glyph = Glyph.Info, onClick = onDetails)
            }
            if (resumeOf != null)
                MediaProgressBar(resumeOf.second.fraction, Modifier.widthIn(max = 240.dp), thickness = 3.dp)
        }
    }
}

@Composable
private fun Shelf(title: String, content: androidx.compose.foundation.lazy.LazyListScope.() -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(title, Modifier.padding(horizontal = MediaTokens.textInset), style = MaterialTheme.typography.titleLarge)
        LazyRow(
            contentPadding = PaddingValues(horizontal = MediaTokens.textInset),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            content = content,
        )
    }
}

@Composable
private fun PosterRow(title: String, titles: List<MovieTitle>, resume: Map<String, MovieResume>, open: (MovieTitle) -> Unit) {
    Shelf(title) {
        items(titles, key = { it.id }) { t ->
            val progress = resume[t.id]?.takeIf(MovieLibrary::shouldResume)?.fraction
            PosterCard(t, progress) { open(t) }
        }
    }
}

@Composable
private fun PosterCard(title: MovieTitle, progress: Float?, onClick: () -> Unit) {
    Column(
        Modifier.width(132.dp)
            .clip(MediaTokens.medium)
            .clickable(role = Role.Button, onClick = onClick)
            .semantics { contentDescription = "${title.title}, ${movieMeta(title)}" }
            .testTag("movie-card-${title.id}"),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Box {
            MoviePoster(title, Modifier.width(132.dp).aspectRatio(2f / 3f))
            if (progress != null)
                MediaProgressBar(
                    progress,
                    Modifier.align(Alignment.BottomCenter).padding(8.dp),
                    thickness = 3.dp,
                    track = Color.White.copy(alpha = 0.3f),
                )
        }
        Text(title.title, style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Text(
            listOfNotNull(title.year.toString(), title.genres.firstOrNull()).joinToString(" · "),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
        )
    }
}

@Composable
private fun ContinueCard(pick: MoviePick, resume: MovieResume, onClick: () -> Unit) {
    Column(
        Modifier.width(220.dp).clip(MediaTokens.medium).clickable(role = Role.Button, onClick = onClick),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Box(Modifier.fillMaxWidth().aspectRatio(16f / 9f)) {
            MoviePoster(pick.title, Modifier.fillMaxSize())
            Box(
                Modifier.fillMaxSize()
                    .clip(MediaTokens.medium)
                    .background(Brush.verticalGradient(0.5f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.7f)))
            )
            Text(
                "${time(resume.positionMs / 1000f)} of ${time(resume.durationMs / 1000f)}",
                Modifier.align(Alignment.BottomStart).padding(start = 10.dp, bottom = 14.dp),
                style = MaterialTheme.typography.labelMedium,
                color = Color.White,
            )
            MediaProgressBar(
                resume.fraction,
                Modifier.align(Alignment.BottomCenter).padding(horizontal = 10.dp, vertical = 6.dp),
                thickness = 3.dp,
                track = Color.White.copy(alpha = 0.3f),
            )
        }
        Text(pick.displayTitle, style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/**
 * Poster through the shared artwork cache, over generated art that also stands in when a title
 * has no poster or it fails to load.
 */
@Composable
internal fun MoviePoster(title: MovieTitle, modifier: Modifier, shape: androidx.compose.ui.graphics.Shape? = MediaTokens.medium) {
    val hue = ((title.id.hashCode() and 0x7fffffff) % 360).toFloat()
    val clipped = if (shape != null) modifier.clip(shape) else modifier
    BoxWithConstraints(
        clipped.background(
            Brush.linearGradient(
                listOf(Color.hsv(hue, 0.55f, 0.42f), Color.hsv((hue + 40f) % 360f, 0.6f, 0.16f))
            )
        )
    ) {
        val pixels = with(LocalDensity.current) { maxWidth.roundToPx() }
        if (title.poster.isBlank()) {
            Column(Modifier.align(Alignment.BottomStart).padding(12.dp)) {
                MediaIcon(Glyph.Film, Modifier.size(28.dp), Color.White.copy(alpha = 0.8f))
                Text(
                    title.title,
                    style = MaterialTheme.typography.titleMedium,
                    color = Color.White,
                    maxLines = 3,
                    fontWeight = FontWeight.SemiBold,
                )
            }
            return@BoxWithConstraints
        }
        val card =
            remember(title.id) {
                MediaCard(JSONObject().put("kind", "video").put("url", "movie:${title.id}").put("thumbnail", title.poster))
            }
        var bitmap by remember(card, pixels) { mutableStateOf(ArtworkCache.cached(card, pixels)) }
        LaunchedEffect(card, pixels) {
            if (bitmap == null)
                bitmap =
                    try {
                        ArtworkCache.load(card, pixels)
                    } catch (cancelled: CancellationException) {
                        throw cancelled
                    } catch (error: Exception) {
                        null
                    }
        }
        bitmap?.let {
            Image(it.asImageBitmap(), null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
        }
            ?: Text(
                title.title,
                Modifier.align(Alignment.BottomStart).padding(12.dp),
                style = MaterialTheme.typography.titleMedium,
                color = Color.White,
            )
    }
}

@Composable
private fun DownloadsSection(
    transfers: Map<String, MovieTransfer>,
    downloads: Map<String, MovieDownload>,
    onOpen: (String) -> Unit,
) {
    val context = LocalContext.current
    Column(Modifier.padding(horizontal = MediaTokens.textInset), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text("Downloads", style = MaterialTheme.typography.titleLarge)
        for (t in transfers.values) TransferRow(t)
        for (d in downloads.values.sortedByDescending { it.savedAt })
            Row(
                Modifier.fillMaxWidth()
                    .clip(MediaTokens.medium)
                    .background(MaterialTheme.colorScheme.surfaceContainer)
                    .clickable { onOpen(d.key) }
                    .padding(start = 12.dp, end = 4.dp, top = 6.dp, bottom = 6.dp)
                    .testTag("movie-download-${d.key}"),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                MediaIcon(Glyph.Download, Modifier.size(20.dp), MaterialTheme.colorScheme.primary)
                Column(Modifier.weight(1f).padding(horizontal = 12.dp)) {
                    Text(d.title, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(
                        listOfNotNull(
                                movieBytes(d.bytes),
                                d.subtitles.takeIf { it > 0 }?.let { if (it == 1) "1 subtitle" else "$it subtitles" },
                                d.source.ifBlank { null },
                            )
                            .joinToString(" · "),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                RoundIconButton(Glyph.Play, "Play ${d.title} offline") { MoviePlayerActivity.offline(context, d) }
            }
    }
}

@Composable
private fun TransferRow(t: MovieTransfer) {
    Column(
        Modifier.fillMaxWidth()
            .clip(MediaTokens.medium)
            .background(MaterialTheme.colorScheme.surfaceContainer)
            .padding(start = 12.dp, end = 4.dp, top = 6.dp, bottom = 10.dp)
            .testTag("movie-transfer-${t.key}"),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(t.title, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(
                    when {
                        t.error.isNotEmpty() -> "Download failed"
                        t.queued -> "Waiting for the current download"
                        else -> "${movieBytes(t.bytes)} · ${t.source}"
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = if (t.error.isNotEmpty()) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (t.error.isNotEmpty()) {
                MovieCatalog.pick(t.key)?.let { pick ->
                    RoundIconButton(Glyph.Refresh, "Retry download") {
                        MovieLibrary.dismiss(t.key)
                        if (pick.title.cinecat) downloadCinecat(pick) else MovieLibrary.download(pick, pick.sources)
                    }
                }
                RoundIconButton(Glyph.Close, "Dismiss") { MovieLibrary.dismiss(t.key) }
            } else RoundIconButton(Glyph.Close, "Cancel download") { MovieLibrary.cancel(t.key) }
        }
        if (t.error.isNotEmpty())
            Text(t.error, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        else MediaProgressBar(null, Modifier.padding(end = 8.dp), thickness = 3.dp)
    }
}

/** Checks [sources] through the engine: the first working index (or null) and every attempt. */
private suspend fun checkSources(sources: List<MovieSource>, all: Boolean): Pair<Int?, List<SourceAttempt>> {
    val data =
        RustRpc.call(
            JSONObject().put("op", "movie_check").put("sources", JSONArray(sources.map { it.json() })).put("all", all)
        ) as? JSONObject ?: return null to emptyList()
    val chosen = if (data.isNull("chosen")) null else data.optInt("chosen")
    return chosen to SourceAttempt.parse(data.optJSONArray("attempts"))
}

/** Resolves a Cinecat title through the sources API: the winner's streams and every attempt. */
private suspend fun resolveCinecat(pick: MoviePick): Pair<List<MovieSource>, List<CinecatReport>> {
    val payload =
        JSONObject()
            .put("op", "movie_cinecat_resolve")
            .put("tmdb", pick.title.tmdb)
    pick.season?.let { payload.put("season", it) }
    pick.episode?.let { payload.put("episode", it) }
    val data = RustRpc.call(payload) as? JSONObject ?: throw IllegalStateException("Empty resolution")
    val sources = parseSources(data.optJSONArray("sources"))
    if (sources.isEmpty()) throw IllegalStateException("No provider served the title.")
    return sources to CinecatReport.parse(data.optJSONArray("attempts"))
}

/** Asks every Cinecat provider for the title and probes samples: how each source did. */
private suspend fun checkAllCinecat(pick: MoviePick): List<CinecatReport> {
    val payload =
        JSONObject()
            .put("op", "movie_cinecat_check_all")
            .put("tmdb", pick.title.tmdb)
            .put("segments", 2)
    pick.season?.let { payload.put("season", it) }
    pick.episode?.let { payload.put("episode", it) }
    val data = RustRpc.call(payload) as? JSONObject ?: throw IllegalStateException("Empty reports")
    return CinecatReport.parse(data.optJSONArray("reports"))
}

/** The engine's resolved Cinecat streams, in the app's source shape. */
private fun parseSources(array: JSONArray?): List<MovieSource> =
    (0 until (array?.length() ?: 0)).mapNotNull { i -> array?.optJSONObject(i)?.let(::movieSourceFromJson) }

/** Resolves a Cinecat pick through the sources API and queues its download. Runs on
 * [MovieLibrary]'s own scope, so a retry keeps going after the failed row leaves the screen. */
internal fun downloadCinecat(pick: MoviePick) {
    MovieLibrary.launch {
        try {
            val (sources, _) = resolveCinecat(pick)
            MovieLibrary.download(pick, sources)
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (error: Exception) {
            MovieLibrary.transfers.update { map ->
                val current = map[pick.key] ?: MovieTransfer(pick.key, pick.displayTitle)
                map + (pick.key to current.copy(queued = false, error = error.message ?: "Could not resolve the title"))
            }
        }
    }
}

/** Pauses the app's own player so a film never plays over music. */
private fun pauseAppPlayback(vm: YtuiViewModel) {
    if (vm.player.value.playing) vm.command("pause")
}

@Composable
private fun MovieSheet(
    vm: YtuiViewModel,
    title: MovieTitle,
    initialSeason: Int?,
    initialEpisode: Int?,
    onDismiss: () -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val resume by MovieLibrary.resume.collectAsState()
    val downloads by MovieLibrary.downloads.collectAsState()
    val transfers by MovieLibrary.transfers.collectAsState()
    var season by remember(title.id) { mutableStateOf(initialSeason ?: title.seasons.firstOrNull()?.number) }
    var episode by remember(title.id) { mutableStateOf(initialEpisode ?: title.seasons.firstOrNull()?.episodes?.firstOrNull()?.number) }
    val pick = MoviePick(title, season.takeIf { title.series }, episode.takeIf { title.series })
    var checking by remember(pick.key) { mutableStateOf(false) }
    var attempts by remember(pick.key) { mutableStateOf<List<SourceAttempt>>(emptyList()) }
    var checkError by remember(pick.key) { mutableStateOf("") }
    var cinecatSources by remember(pick.key) { mutableStateOf<List<MovieSource>>(emptyList()) }
    var cinecatReports by remember(pick.key) { mutableStateOf<List<CinecatReport>>(emptyList()) }
    /** The running check/resolve; cancelled when the pick changes so it never plays a stale episode. */
    var resolveJob by remember(pick.key) { mutableStateOf<Job?>(null) }
    val saved = resume[pick.key]?.takeIf(MovieLibrary::shouldResume)
    val download = downloads[pick.key]
    val transfer = transfers[pick.key]

    fun check(all: Boolean, then: ((Int) -> Unit)? = null) {
        if (checking) return
        checking = true
        checkError = ""
        attempts = emptyList()
        resolveJob?.cancel()
        resolveJob = scope.launch {
            try {
                val (chosen, result) = checkSources(pick.sources, all)
                attempts = result
                if (chosen == null) checkError = "No source delivered video. Check your connection, then try again."
                else then?.invoke(chosen)
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Exception) {
                checkError = error.message ?: "Could not check sources"
            } finally {
                checking = false
            }
        }
    }

    // Cinecat titles resolve through the sources API: Play asks for the winner with provider
    // failover, Check all qualifies every provider the way the standalone CLI's check does.
    fun resolve(then: (List<MovieSource>) -> Unit) {
        // A previous resolve for this pick already succeeded: reuse its streams.
        if (cinecatSources.isNotEmpty()) {
            then(cinecatSources)
            return
        }
        if (checking) return
        checking = true
        checkError = ""
        cinecatReports = emptyList()
        resolveJob?.cancel()
        resolveJob = scope.launch {
            try {
                val (sources, reports) = resolveCinecat(pick)
                cinecatSources = sources
                cinecatReports = reports
                then(sources)
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Exception) {
                checkError = error.message ?: "Could not resolve the title"
            } finally {
                checking = false
            }
        }
    }

    fun checkCinecatAll() {
        if (checking) return
        checking = true
        checkError = ""
        cinecatReports = emptyList()
        resolveJob?.cancel()
        resolveJob = scope.launch {
            try {
                cinecatReports = checkAllCinecat(pick)
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Exception) {
                checkError = error.message ?: "Could not check providers"
            } finally {
                checking = false
            }
        }
    }

    Column(
        Modifier.fillMaxWidth()
            .heightIn(max = LocalConfiguration.current.screenHeightDp.dp * 0.9f)
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 20.dp)
            .padding(bottom = 24.dp)
            .testTag("movie-sheet"),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            MoviePoster(title, Modifier.width(112.dp).aspectRatio(2f / 3f))
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Row(verticalAlignment = Alignment.Top) {
                    Text(title.title, Modifier.weight(1f), style = MaterialTheme.typography.headlineSmall)
                    SheetClose(onDismiss)
                }
                Text(movieMeta(title), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    for (feature in title.features.take(3)) Tag(feature)
                }
            }
        }
        Text(title.synopsis, style = MaterialTheme.typography.bodyLarge)
        Text(
            "${title.credit} · ${title.license}",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (title.series) EpisodePicker(title, season, episode, resume) { s, e ->
            resolveJob?.cancel()
            season = s
            episode = e
        }
        // Actions: play (offline copy first), download state, and a full source check.
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            if (download != null)
                Pill("Play download", Modifier.testTag("movie-play-offline"), PillStyle.Primary, glyph = Glyph.Play) {
                    pauseAppPlayback(vm)
                    MoviePlayerActivity.offline(context, download)
                }
            else
                Pill(
                    when {
                        checking -> if (title.cinecat) "Finding sources…" else "Checking sources…"
                        saved != null -> "Resume from ${time(saved.positionMs / 1000f)}"
                        else -> "Play"
                    },
                    Modifier.testTag("movie-play"),
                    PillStyle.Primary,
                    enabled = !checking,
                    glyph = Glyph.Play,
                ) {
                    if (title.cinecat)
                        resolve { sources ->
                            pauseAppPlayback(vm)
                            MoviePlayerActivity.stream(context, pick.key, pick.displayTitle, sources)
                        }
                    else
                        check(all = false) { chosen ->
                            pauseAppPlayback(vm)
                            MoviePlayerActivity.stream(context, pick.key, pick.displayTitle, movieFailoverOrder(pick.sources, chosen))
                        }
                }
            when {
                download != null ->
                    Pill("Remove", style = PillStyle.Destructive) { MovieLibrary.delete(pick.key) }
                transfer != null && transfer.error.isEmpty() ->
                    Pill("Cancel download", style = PillStyle.Quiet) { MovieLibrary.cancel(pick.key) }
                else ->
                    Pill("Download", Modifier.testTag("movie-download"), enabled = !checking, glyph = Glyph.Download) {
                        if (title.cinecat)
                            resolve { sources -> MovieLibrary.download(pick, sources) }
                        else {
                            MovieLibrary.dismiss(pick.key)
                            MovieLibrary.download(pick, movieFailoverOrder(pick.sources, attempts.firstOrNull { it.ok }?.index))
                        }
                    }
            }
        }
        if (download != null)
            Pill("Stream instead", style = PillStyle.Quiet, enabled = !checking) {
                if (title.cinecat)
                    resolve { sources ->
                        pauseAppPlayback(vm)
                        MoviePlayerActivity.stream(context, pick.key, pick.displayTitle, sources)
                    }
                else
                    check(all = false) { chosen ->
                        pauseAppPlayback(vm)
                        MoviePlayerActivity.stream(context, pick.key, pick.displayTitle, movieFailoverOrder(pick.sources, chosen))
                    }
            }
        if (transfer != null) TransferRow(transfer)
        if (download != null)
            Text(
                "Saved on this device · ${movieBytes(download.bytes)} from ${download.source}",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        if (title.cinecat)
            CinecatProviders(cinecatReports, checking, checkError) { checkCinecatAll() }
        else SourceList(pick.sources, attempts, checking, checkError) { check(all = true) }
    }
}

@Composable
private fun Tag(text: String) {
    Text(
        text,
        Modifier.clip(MediaTokens.small)
            .border(1.dp, MaterialTheme.colorScheme.outlineVariant, MediaTokens.small)
            .padding(horizontal = 8.dp, vertical = 3.dp),
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        maxLines = 1,
    )
}

@Composable
private fun EpisodePicker(
    title: MovieTitle,
    season: Int?,
    episode: Int?,
    resume: Map<String, MovieResume>,
    choose: (Int, Int) -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            for (s in title.seasons)
                MediaChip("Season ${s.number}", s.number == season, Modifier.testTag("movie-season-${s.number}")) {
                    choose(s.number, s.episodes.first().number)
                }
        }
        val current = title.seasons.firstOrNull { it.number == season } ?: return
        for (e in current.episodes) {
            val selected = e.number == episode
            val progress = resume["${title.id}:${current.number}:${e.number}"]?.takeIf(MovieLibrary::shouldResume)
            Row(
                Modifier.fillMaxWidth()
                    .clip(MediaTokens.medium)
                    .background(if (selected) MaterialTheme.colorScheme.surfaceContainerHigh else Color.Transparent)
                    .clickable(role = Role.RadioButton) { choose(current.number, e.number) }
                    .padding(10.dp)
                    .testTag("movie-episode-${current.number}-${e.number}"),
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text(
                    "${e.number}",
                    Modifier.width(24.dp),
                    style = MaterialTheme.typography.titleLarge,
                    color = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Row {
                        Text(e.title, Modifier.weight(1f), style = MaterialTheme.typography.titleSmall)
                        Text(
                            movieRuntime(e.runtimeMinutes),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    Text(
                        e.synopsis,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = if (selected) 4 else 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                    if (progress != null) MediaProgressBar(progress.fraction, Modifier.padding(top = 4.dp), thickness = 3.dp)
                }
            }
        }
    }
}

/** Each source with its last check result: what failed, and what the chosen one delivered. */
@Composable
private fun SourceList(
    sources: List<MovieSource>,
    attempts: List<SourceAttempt>,
    checking: Boolean,
    error: String,
    checkAll: () -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.testTag("movie-sources")) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("Sources", Modifier.weight(1f), style = MaterialTheme.typography.titleMedium)
            Pill(if (checking) "Checking…" else "Check all", style = PillStyle.Quiet, enabled = !checking, onClick = checkAll)
        }
        val chosen = attempts.firstOrNull { it.ok }?.index
        sources.forEachIndexed { i, source ->
            val attempt = attempts.firstOrNull { it.index == i }
            Row(
                Modifier.fillMaxWidth()
                    .clip(MediaTokens.medium)
                    .background(MaterialTheme.colorScheme.surfaceContainer)
                    .padding(horizontal = 12.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Box(Modifier.size(22.dp), contentAlignment = Alignment.Center) {
                    when {
                        attempt == null && checking -> MediaSpinner(Modifier.size(18.dp))
                        attempt == null -> Text("${i + 1}", style = MaterialTheme.typography.labelLarge)
                        attempt.ok -> MediaIcon(Glyph.Check, Modifier.size(20.dp), MaterialTheme.colorScheme.primary)
                        else -> MediaIcon(Glyph.Close, Modifier.size(20.dp), MaterialTheme.colorScheme.error)
                    }
                }
                Column(Modifier.weight(1f)) {
                    Text(
                        source.label + if (i == chosen) " · used" else "",
                        style = MaterialTheme.typography.bodyMedium,
                        fontWeight = if (i == chosen) FontWeight.SemiBold else FontWeight.Normal,
                    )
                    Text(
                        attempt?.detail ?: if (source.kind == "hls") "HLS stream" else "Video file",
                        style = MaterialTheme.typography.bodySmall,
                        color =
                            if (attempt != null && !attempt.ok) MaterialTheme.colorScheme.error
                            else MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
        if (error.isNotEmpty())
            Text(error, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
    }
}

/**
 * Each Cinecat provider with its last result: the failover attempts from a resolve, or the
 * per-provider qualification from Check all. Mirrors [SourceList]'s visual language.
 */
@Composable
private fun CinecatProviders(
    reports: List<CinecatReport>,
    checking: Boolean,
    error: String,
    checkAll: () -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.testTag("movie-sources")) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("Providers", Modifier.weight(1f), style = MaterialTheme.typography.titleMedium)
            Pill(if (checking) "Checking…" else "Check all", style = PillStyle.Quiet, enabled = !checking, onClick = checkAll)
        }
        if (reports.isEmpty() && !checking)
            Text(
                "Play asks the Cinecat sources API for this title and fails over through its providers.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        if (reports.isEmpty() && checking)
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                MediaSpinner(Modifier.size(18.dp), MaterialTheme.colorScheme.onSurfaceVariant)
                Text(
                    "Asking each provider…",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        for (report in reports)
            Row(
                Modifier.fillMaxWidth()
                    .clip(MediaTokens.medium)
                    .background(MaterialTheme.colorScheme.surfaceContainer)
                    .padding(horizontal = 12.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Box(Modifier.size(22.dp), contentAlignment = Alignment.Center) {
                    if (report.ok) MediaIcon(Glyph.Check, Modifier.size(20.dp), MaterialTheme.colorScheme.primary)
                    else MediaIcon(Glyph.Close, Modifier.size(20.dp), MaterialTheme.colorScheme.error)
                }
                Column(Modifier.weight(1f)) {
                    Text(report.provider, style = MaterialTheme.typography.bodyMedium)
                    Text(
                        report.detail,
                        style = MaterialTheme.typography.bodySmall,
                        color =
                            if (report.ok) MaterialTheme.colorScheme.onSurfaceVariant
                            else MaterialTheme.colorScheme.error,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        if (error.isNotEmpty())
            Text(error, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
    }
}

/** Plays or downloads any HLS or video-file URL the user enters, through the same engine path. */
@Composable
private fun StreamUrlPanel(vm: YtuiViewModel) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var url by rememberSaveable { mutableStateOf("") }
    var kind by rememberSaveable { mutableStateOf<String?>(null) }
    var checking by remember { mutableStateOf(false) }
    var status by remember { mutableStateOf("") }
    val trimmed = url.trim()
    val valid = movieUrlValid(trimmed)
    val effectiveKind = kind ?: movieKindOf(trimmed)
    val key = "url:" + Integer.toHexString(trimmed.hashCode())
    val label = runCatching { java.net.URI(trimmed).let { (it.host ?: "") + (it.path ?: "") } }.getOrDefault(trimmed).take(80)
    val source = MovieSource(label.substringBefore('/').ifBlank { "Stream URL" }, effectiveKind, trimmed)
    fun play() {
        if (!valid || checking) return
        checking = true
        status = ""
        scope.launch {
            try {
                val (chosen, attempts) = checkSources(listOf(source), false)
                if (chosen == null) status = attempts.firstOrNull()?.detail ?: "The stream did not deliver video."
                else {
                    pauseAppPlayback(vm)
                    MoviePlayerActivity.stream(context, key, label, listOf(source))
                }
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (error: Exception) {
                status = error.message ?: "Could not check the stream"
            } finally {
                checking = false
            }
        }
    }
    Column(
        Modifier.padding(horizontal = MediaTokens.textInset)
            .fillMaxWidth()
            .clip(MediaTokens.card)
            .background(MaterialTheme.colorScheme.surfaceContainer)
            .padding(16.dp)
            .testTag("movies-url"),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Text("Play a stream URL", style = MaterialTheme.typography.titleMedium)
        Text(
            "Paste an HLS playlist (.m3u8) or a video file link you have the right to watch.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        MediaTextField(
            url,
            {
                url = it
                status = ""
            },
            "https://…",
            Modifier.fillMaxWidth().testTag("movies-url-field"),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go),
            keyboardActions = KeyboardActions(onGo = { play() }),
        )
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            MediaChip("HLS", effectiveKind == "hls") { kind = "hls" }
            MediaChip("Video file", effectiveKind == "file") { kind = "file" }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Pill(if (checking) "Checking…" else "Play", style = PillStyle.Primary, enabled = valid && !checking, glyph = Glyph.Play, onClick = ::play)
            Pill("Download", enabled = valid, glyph = Glyph.Download) {
                MovieLibrary.download(key, label, listOf(source))
                status = "Added to Downloads."
            }
        }
        if (url.isNotBlank() && !valid)
            Text("Enter a full http(s) link.", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
        if (status.isNotEmpty())
            Text(status, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

