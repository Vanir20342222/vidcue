package dev.ytui.android

import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import org.junit.Assert.*
import org.junit.Test

class ArtworkLoaderTest {
    @Test
    fun callersOnDifferentThreadsShareEachCover() = runBlocking {
        withTimeout(3_000) {
            val memory = ConcurrentHashMap<Int, String>()
            val calls = ConcurrentHashMap<Int, AtomicInteger>()
            val loader =
                ArtworkLoader<Int, String>(
                    CoroutineScope(coroutineContext + Dispatchers.Default),
                    memory::get,
                    { k, v -> memory[k] = v },
                )
            val start = CompletableDeferred<Unit>()
            val viewers =
                List(100) { index ->
                    async(Dispatchers.Default) {
                        start.await()
                        val key = index % 4
                        assertEquals(
                            "pixels-$key",
                            loader.load(key) {
                                calls.computeIfAbsent(key) { AtomicInteger() }.incrementAndGet()
                                delay(10)
                                "pixels-$key"
                            },
                        )
                    }
                }
            start.complete(Unit)
            viewers.awaitAll()
            assertEquals(4, calls.size)
            calls.values.forEach { assertEquals(1, it.get()) }
        }
    }

    @Test
    fun simultaneousCardsShareOneDecodeAndCacheHitNeedsNoWork() = runBlocking {
        withTimeout(3_000) {
            val memory = ConcurrentHashMap<String, String>()
            val loader = ArtworkLoader<String, String>(this, memory::get, { k, v -> memory[k] = v })
            val response = CompletableDeferred<String>()
            var calls = 0
            val first = async {
                loader.load("cover") {
                    calls++
                    response.await()
                }
            }
            yield()
            val second = async { loader.load("cover") { error("duplicate decode") } }
            yield()
            response.complete("pixels")
            assertEquals("pixels", first.await())
            assertEquals("pixels", second.await())
            assertEquals("pixels", loader.load("cover") { error("cache miss") })
            assertEquals(1, calls)
        }
    }

    @Test
    fun scrollAwayAndBackJoinsRunningDecode() = runBlocking {
        withTimeout(3_000) {
            val memory = ConcurrentHashMap<String, String>()
            val loader = ArtworkLoader<String, String>(this, memory::get, { k, v -> memory[k] = v })
            val started = CompletableDeferred<Unit>()
            val response = CompletableDeferred<String>()
            val first = launch {
                loader.load("cover") {
                    started.complete(Unit)
                    response.await()
                }
            }
            started.await()
            first.cancelAndJoin()
            val returned = async { loader.load("cover") { error("duplicate JNI call") } }
            yield()
            response.complete("pixels")
            assertEquals("pixels", returned.await())
        }
    }

    @Test
    fun completedDecodeWarmsCacheAfterAllCardsLeave() = runBlocking {
        withTimeout(3_000) {
            val stored = CompletableDeferred<String>()
            val loader =
                ArtworkLoader<String, String>(this, { null }, { _, v -> stored.complete(v) })
            val started = CompletableDeferred<Unit>()
            val response = CompletableDeferred<String>()
            val card = launch {
                loader.load("cover") {
                    started.complete(Unit)
                    response.await()
                }
            }
            started.await()
            card.cancelAndJoin()
            response.complete("pixels")
            assertEquals("pixels", stored.await())
        }
    }

    @Test
    fun abandonedQueuedCardsNeverStartAndDoNotBlockNewVisibleCards() = runBlocking {
        withTimeout(3_000) {
            val loader = ArtworkLoader<String, String>(this, { null }, { _, _ -> }, parallelism = 1)
            val started = CompletableDeferred<Unit>()
            val response = CompletableDeferred<String>()
            val running = async {
                loader.load("running") {
                    started.complete(Unit)
                    response.await()
                }
            }
            started.await()
            repeat(30) { index ->
                val queued = launch { loader.load("skip-$index") { error("offscreen decode") } }
                yield()
                queued.cancelAndJoin()
            }
            val visible = async { loader.load("visible") { "visible pixels" } }
            response.complete("first pixels")
            assertEquals("first pixels", running.await())
            assertEquals("visible pixels", visible.await())
            assertEquals("returned pixels", loader.load("skip-0") { "returned pixels" })
        }
    }

    @Test
    fun cancelledWaiterDoesNotCancelOtherCards() = runBlocking {
        withTimeout(3_000) {
            val loader = ArtworkLoader<String, String>(this, { null }, { _, _ -> })
            val started = CompletableDeferred<Unit>()
            val response = CompletableDeferred<String>()
            val first = launch {
                loader.load("cover") {
                    started.complete(Unit)
                    response.await()
                }
            }
            started.await()
            val second = async { loader.load("cover") { error("duplicate") } }
            yield()
            first.cancelAndJoin()
            response.complete("pixels")
            assertEquals("pixels", second.await())
        }
    }

    @Test
    fun orphanedDecodesStillCountAgainstConcurrencyLimit() = runBlocking {
        withTimeout(3_000) {
            val loader = ArtworkLoader<Int, String>(this, { null }, { _, _ -> }, parallelism = 2)
            val started = Channel<Int>(Channel.UNLIMITED)
            val responses = List(3) { CompletableDeferred<String>() }
            val cards =
                (0..1).map { index ->
                    launch {
                        loader.load(index) {
                            started.send(index)
                            responses[index].await()
                        }
                    }
                }
            repeat(2) { started.receive() }
            cards.forEach { it.cancelAndJoin() }
            val third = async {
                loader.load(2) {
                    started.send(2)
                    responses[2].await()
                }
            }
            yield()
            assertTrue(started.tryReceive().isFailure)
            responses[0].complete("first")
            assertEquals(2, started.receive())
            responses[1].complete("second")
            responses[2].complete("third")
            assertEquals("third", third.await())
        }
    }

    @Test
    fun nullAndFailedLoadsCanRetryWithoutAStaleFlight() = runBlocking {
        supervisorScope {
            val loader = ArtworkLoader<String, String>(this, { null }, { _, _ -> })
            assertNull(loader.load("cover") { null })
            try {
                loader.load("cover") { throw IllegalStateException("temporary failure") }
                fail("expected failure")
            } catch (_: IllegalStateException) {}
            assertEquals("recovered", loader.load("cover") { "recovered" })
        }
    }
}
