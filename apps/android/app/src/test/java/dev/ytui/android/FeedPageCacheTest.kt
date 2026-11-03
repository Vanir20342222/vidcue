package dev.ytui.android

import kotlinx.coroutines.*
import org.junit.Assert.*
import org.junit.Test

class FeedPageCacheTest {
    @Test
    fun preparedCategoryOpensWithoutAnotherFetchAndExpires() = runBlocking {
        var now = 0L
        var calls = 0
        val cache = FeedPageCache<String, String>(this, { now }, { it.length })
        suspend fun open() =
            cache.load("video:music") {
                calls++
                "prepared cards"
            }
        assertEquals("prepared cards", open())
        assertEquals("prepared cards", open())
        assertEquals(1, calls)
        now = 60_000
        assertEquals("prepared cards", open())
        assertEquals(2, calls)
    }

    @Test
    fun tapJoinsPreloadEvenAfterPreloadWaiterIsCancelled() = runBlocking {
        val cache = FeedPageCache<String, String>(this, { 0 }, { it.length })
        val started = CompletableDeferred<Unit>()
        val response = CompletableDeferred<String>()
        val preload = launch {
            cache.load("music:songs") {
                started.complete(Unit)
                response.await()
            }
        }
        started.await()
        preload.cancelAndJoin()
        val tap = async { cache.load("music:songs") { error("duplicate request") } }
        yield()
        response.complete("songs")
        assertEquals("songs", tap.await())
    }

    @Test
    fun accountChangeCancelsRequestsAndDropsPrivatePages() = runBlocking {
        val cache = FeedPageCache<String, String>(this, { 0 }, { it.length })
        cache.load("liked") { "private cards" }
        val started = CompletableDeferred<Unit>()
        val pending = launch {
            cache.load("playlists") {
                started.complete(Unit)
                awaitCancellation()
            }
        }
        started.await()
        cache.clear()
        pending.join()
        assertTrue(pending.isCancelled)
        assertNull(cache.peek("liked"))
        assertEquals("new account", cache.load("playlists") { "new account" })
    }

    @Test
    fun failuresAreNotCachedAndDoNotBlockForegroundRetry() = runBlocking {
        supervisorScope {
            val cache = FeedPageCache<String, String>(this, { 0 }, { it.length })
            try {
                cache.load("category") { throw IllegalStateException("cooldown") }
                fail("expected failure")
            } catch (_: IllegalStateException) {}
            assertEquals("recovered", cache.load("category") { "recovered" })
        }
    }

    @Test
    fun cacheEvictsLeastRecentlyUsedPagesAndRejectsOversizedPages() = runBlocking {
        val cache =
            FeedPageCache<String, String>(this, { 0 }, { it.length }, maxEntries = 2, maxBytes = 6)
        cache.load("a") { "aaa" }
        cache.load("b") { "bbb" }
        assertEquals("aaa", cache.peek("a"))
        cache.load("c") { "ccc" }
        assertNull(cache.peek("b"))
        assertEquals("oversized", cache.load("large") { "oversized" })
        assertNull(cache.peek("large"))
        assertEquals("aaa", cache.peek("a"))
        assertEquals("ccc", cache.peek("c"))
    }

    @Test
    fun explicitRefreshReplacesCachedPageAndTabsStaySeparate() = runBlocking {
        val cache = FeedPageCache<Pair<Int, String>, String>(this, { 0 }, { it.length })
        cache.load(1 to "Home") { "videos" }
        cache.load(2 to "Home") { "music" }
        cache.invalidate(1 to "Home")
        assertEquals("fresh videos", cache.load(1 to "Home") { "fresh videos" })
        assertEquals("music", cache.load(2 to "Home") { error("music was evicted") })
    }

    @Test
    fun invalidatedReplyCannotReplaceNewAccountData() = runBlocking {
        val cache = FeedPageCache<String, String>(this, { 0 }, { it.length })
        val started = CompletableDeferred<Unit>()
        val oldResponse = CompletableDeferred<Unit>()
        val old = launch {
            cache.load("Home") {
                withContext(NonCancellable) {
                    started.complete(Unit)
                    oldResponse.await()
                }
                "old account"
            }
        }
        started.await()
        cache.clear()
        assertEquals("new account", cache.load("Home") { "new account" })
        oldResponse.complete(Unit)
        old.join()
        assertEquals("new account", cache.peek("Home"))
    }

    @Test
    fun rapidNavigationEvictsAbandonedFetchesWithoutCancellingVisibleCategory() = runBlocking {
        val cache = FeedPageCache<String, String>(this, { 0 }, { it.length })
        val response = CompletableDeferred<String>()
        val started = CompletableDeferred<Unit>()
        val visible = async {
            cache.load("visible") {
                started.complete(Unit)
                response.await()
            }
        }
        started.await()
        repeat(8) { index ->
            val fetching = CompletableDeferred<Unit>()
            val abandoned = launch {
                cache.load("abandoned:$index") {
                    fetching.complete(Unit)
                    awaitCancellation()
                }
            }
            fetching.await()
            abandoned.cancelAndJoin()
        }
        response.complete("visible cards")
        assertEquals("visible cards", visible.await())
        cache.clear()
    }

    @Test
    fun byteBudgetEvictsPagesBeforeEntryLimit() = runBlocking {
        val cache =
            FeedPageCache<String, String>(this, { 0 }, { it.length }, maxEntries = 12, maxBytes = 5)
        cache.load("first") { "aaa" }
        cache.load("second") { "bbb" }
        assertNull(cache.peek("first"))
        assertEquals("bbb", cache.peek("second"))
    }
}
