package dev.ytui.android

import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import org.junit.Assert.*
import org.junit.Test

class FeedPagePreloaderTest {
    @Test(timeout = 10_000)
    fun disablingPreloadingDuringBatchPreventsFurtherRequests() = runBlocking {
        val cache = FeedPageCache<Int, String>(this, { 0 }, { it.length })
        val preloader = FeedPagePreloader(cache)
        val started = Channel<Int>(Channel.UNLIMITED)
        val response = CompletableDeferred<Unit>()
        var allowed = true
        val batch = launch {
            preloader.prepare((0..23).toList(), { it }, { allowed }) { index ->
                started.send(index)
                response.await()
                "category $index"
            }
        }
        started.receive()
        started.receive()
        allowed = false
        response.complete(Unit)
        batch.join()
        assertTrue(started.tryReceive().isFailure)
    }

    @Test(timeout = 10_000)
    fun preparesTheWholeStripWithOnlyTwoConcurrentFetches() = runBlocking {
        val cache = FeedPageCache<Int, String>(this, { 0 }, { it.length }, maxEntries = 32)
        val preloader = FeedPagePreloader(cache)
        val started = Channel<Int>(Channel.UNLIMITED)
        val permits = Channel<Unit>()
        var active = 0
        var peak = 0
        val batch = launch {
            preloader.prepare((0..23).toList(), { it }, { true }) { index ->
                active++
                peak = maxOf(peak, active)
                started.send(index)
                permits.receive()
                active--
                "category $index"
            }
        }
        assertEquals(setOf(0, 1), setOf(started.receive(), started.receive()))
        assertTrue(started.tryReceive().isFailure)
        repeat(24) { permits.send(Unit) }
        batch.join()
        assertEquals(2, peak)
        repeat(24) { assertEquals("category $it", cache.peek(it)) }
    }

    @Test(timeout = 10_000)
    fun replacementBatchesShareLimitAndForegroundJoinsAbandonedPreload() = runBlocking {
        val cache = FeedPageCache<Int, String>(this, { 0 }, { it.length })
        val preloader = FeedPagePreloader(cache)
        val started = Channel<Int>(Channel.UNLIMITED)
        val response = CompletableDeferred<Unit>()
        suspend fun fetch(index: Int): String {
            started.send(index)
            response.await()
            return "category $index"
        }
        val first = launch { preloader.prepare(listOf(1, 2), { it }, { true }, ::fetch) }
        started.receive()
        started.receive()
        first.cancelAndJoin()
        val next = launch { preloader.prepare(listOf(3, 4), { it }, { true }, ::fetch) }
        yield()
        assertTrue(started.tryReceive().isFailure)
        val tap = async { cache.load(2) { error("duplicate foreground fetch") } }
        response.complete(Unit)
        assertEquals("category 2", tap.await())
        next.join()
        assertEquals(setOf(3, 4), setOf(started.receive(), started.receive()))
    }

    @Test(timeout = 10_000)
    fun disabledPreloadingDoesNoWorkAndFailureStopsTheStrip() = runBlocking {
        supervisorScope {
            val cache = FeedPageCache<Int, String>(this, { 0 }, { it.length })
            val preloader = FeedPagePreloader(cache)
            preloader.prepare(listOf(1, 2), { it }, { false }) { error("preloading disabled") }
            var calls = 0
            preloader.prepare((0..23).toList(), { it }, { true }) {
                calls++
                error("service cooldown")
            }
            assertTrue(calls in 1..2)
        }
    }
}
