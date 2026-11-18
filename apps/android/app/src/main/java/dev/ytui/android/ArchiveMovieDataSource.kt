package dev.ytui.android

import android.net.Uri
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DataSpec
import java.io.IOException
import java.io.InterruptedIOException

/** Keeps the current Archive CDN URL for range reads within one progressive movie load. */
@androidx.annotation.OptIn(markerClass = [androidx.media3.common.util.UnstableApi::class])
internal class ArchiveMovieDataSource(
    private val upstream: DataSource,
    private val files: Set<Uri>,
) : DataSource by upstream {
    private var original: Uri? = null
    private var redirect: Uri? = null

    override fun open(dataSpec: DataSpec): Long {
        val previous = redirect
        if (previous != null && dataSpec.uri == original && dataSpec.position > 0) {
            try {
                return upstream.open(dataSpec.withUri(previous))
            } catch (interrupted: InterruptedIOException) {
                throw interrupted
            } catch (unavailable: IOException) {
                // An expired or unavailable CDN should be reselected through the catalog.
                upstream.close()
            }
        }
        original = null
        redirect = null
        val length = upstream.open(dataSpec)
        val finalUri = upstream.uri
        if (dataSpec.uri in files && finalUri?.scheme == "https" &&
            finalUri.host?.endsWith(".archive.org") == true && finalUri != dataSpec.uri
        ) {
            original = dataSpec.uri
            redirect = finalUri
        }
        return length
    }
}
