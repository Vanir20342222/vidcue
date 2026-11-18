package dev.ytui.android

import android.net.Uri
import androidx.media3.datasource.ByteArrayDataSource
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DataSpec
import java.io.IOException
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(manifest = Config.NONE, sdk = [35])
@androidx.annotation.OptIn(markerClass = [androidx.media3.common.util.UnstableApi::class])
class ArchiveMovieDataSourceTest {
    @Test
    fun staleCdnRetriesCatalogForTheSameRangeAndNeverRedirectsAnotherFile() {
        val catalog = Uri.parse("https://archive.org/download/example/movie.mp4")
        val cdn = Uri.parse("https://storage.archive.org/movie.mp4")
        val other = Uri.parse("https://example.org/subtitle.vtt")
        val requests = mutableListOf<Pair<Uri, Long>>()
        val upstream = object : DataSource by ByteArrayDataSource(ByteArray(32)) {
            override fun getUri(): Uri = cdn
            override fun close() {}
            override fun open(dataSpec: DataSpec): Long {
                requests.add(dataSpec.uri to dataSpec.position)
                if (dataSpec.uri == cdn) throw IOException("expired")
                return 32L - dataSpec.position
            }
        }
        val source = ArchiveMovieDataSource(upstream, setOf(catalog))
        for ((url, offset) in listOf(catalog to 0L, catalog to 16L, other to 4L)) {
            source.open(DataSpec.Builder().setUri(url).setPosition(offset).build())
            source.close()
        }
        assertEquals(listOf(catalog to 0L, cdn to 16L, catalog to 16L, other to 4L), requests)
    }
}
