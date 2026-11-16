package dev.ytui.android

import android.net.Uri
import android.os.SystemClock
import androidx.media3.common.C
import androidx.media3.datasource.DataSpec
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.net.InetAddress
import java.net.ServerSocket
import java.net.Socket
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import org.junit.Assert.*
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith

/** Controlled transport throughput, explicitly separate from live CDN qualification. */
@RunWith(AndroidJUnit4::class)
class DownloadThroughputTest {
    @Test
    fun boundedRangeConcurrencyPreservesBytesAndReportsThroughput() {
        assumeTrue(InstrumentationRegistry.getArguments().getString("vidcue_download_throughput") == "measure")
        val report = StringBuilder("Synthetic loopback range server; 40 ms/request, 4 ms/64 KiB per connection. Not live CDN throughput.\n")
        val workers = Executors.newFixedThreadPool(8)
        try {
            RangeServer().use { server ->
                for (sample in 0 until 3) for (transport in listOf("current", "pooled")) for (connections in listOf(1, 2, 4, 8)) {
                    val source = ChunkedRangeDataSource(
                        if (transport == "current") androidx.media3.datasource.DefaultHttpDataSource.Factory()
                            .setConnectTimeoutMs(10_000).setReadTimeoutMs(20_000)
                            .setAllowCrossProtocolRedirects(false)
                        else PlaybackHttp.factory(emptyMap()), { true },
                        ChunkedRangeDataSource.MIN_CHUNK_BYTES,
                        minOf(ChunkedRangeDataSource.MAX_CHUNK_BYTES, ChunkedRangeDataSource.BUFFER_BYTES / connections),
                        connections, workers,
                    )
                    val spec = DataSpec.Builder().setUri(Uri.parse(server.url)).setLength(server.size.toLong()).build()
                    var received = 0L
                    val checksum = java.util.zip.CRC32()
                    val buffer = ByteArray(64 * 1024)
                    val start = SystemClock.elapsedRealtimeNanos()
                    try {
                        assertEquals(server.size.toLong(), source.open(spec))
                        while (true) {
                            val n = source.read(buffer, 0, buffer.size)
                            if (n == C.RESULT_END_OF_INPUT) break
                            assertTrue("A read must make progress", n > 0)
                            checksum.update(buffer, 0, n)
                            received += n
                        }
                    } finally { source.close() }
                    val elapsed = (SystemClock.elapsedRealtimeNanos() - start) / 1_000_000.0
                    assertEquals(server.size.toLong(), received)
                    assertEquals("Range bytes must stay complete and ordered", server.checksum, checksum.value)
                    report.append("sample=$sample transport=$transport connections=$connections bytes=$received elapsed_ms=$elapsed mib_per_second=${received * 1000.0 / elapsed / 1048576.0}\n")
                }
            }
        } finally {
            workers.shutdownNow()
            workers.awaitTermination(5, TimeUnit.SECONDS)
            File(InstrumentationRegistry.getInstrumentation().targetContext.getExternalFilesDir(null), "download-throughput.txt").writeText(report.toString())
        }
    }

    private class RangeServer : AutoCloseable {
        val size = 8 * 1024 * 1024
        private val bytes = ByteArray(size) { (it % 251).toByte() }
        val checksum = java.util.zip.CRC32().apply { update(bytes) }.value
        private val listener = ServerSocket(0, 16, InetAddress.getByName("127.0.0.1"))
        private val clients = java.util.concurrent.ConcurrentHashMap.newKeySet<Socket>()
        private val workers = Executors.newFixedThreadPool(8)
        val url = "http://127.0.0.1:${listener.localPort}/synthetic"
        private val acceptor = Thread({
            while (!listener.isClosed) {
                val socket = try { listener.accept() } catch (_: java.io.IOException) { break }
                clients.add(socket)
                workers.execute {
                    try { socket.use { serve(it) } } catch (_: java.io.IOException) { }
                    finally { clients.remove(socket) }
                }
            }
        }, "throughput-fixture").apply { start() }

        private fun serve(socket: Socket) {
            socket.soTimeout = 10_000
            val reader = socket.getInputStream().bufferedReader(Charsets.US_ASCII)
            reader.readLine() ?: return
            var start = 0
            var end = size - 1
            while (true) {
                val line = reader.readLine() ?: return
                if (line.isEmpty()) break
                if (line.startsWith("Range: bytes=", ignoreCase = true)) {
                    val range = line.substringAfter('=').split('-')
                    start = range[0].toInt()
                    if (range[1].isNotEmpty()) end = minOf(end, range[1].toInt())
                }
            }
            require(start in 0 until size && end in start until size)
            Thread.sleep(40)
            val output = socket.getOutputStream()
            output.write(("HTTP/1.1 206 Partial Content\r\nContent-Length: ${end - start + 1}\r\n" +
                "Content-Range: bytes $start-$end/$size\r\nConnection: close\r\n\r\n").toByteArray(Charsets.US_ASCII))
            var position = start
            while (position <= end) {
                val n = minOf(64 * 1024, end - position + 1)
                Thread.sleep(4)
                output.write(bytes, position, n)
                position += n
            }
            output.flush()
        }

        override fun close() {
            listener.close()
            clients.forEach { runCatching { it.close() } }
            workers.shutdownNow()
            acceptor.join(2000)
            workers.awaitTermination(5, TimeUnit.SECONDS)
        }
    }
}
