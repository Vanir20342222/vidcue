package dev.ytui.android

import kotlinx.coroutines.*

/** Main-dispatcher confined. A cancelled screen can leave one useful fetch for the next screen. */
internal class FeedPageCache<K, V>(
    private val scope: CoroutineScope,
    private val clock: () -> Long,
    private val sizeOf: (V) -> Int,
    private val ttlMs: Long = 60_000,
    private val maxEntries: Int = 12,
    private val maxBytes: Int = 2 * 1024 * 1024,
) {
    private data class Entry<V>(val value: V, val expires: Long, val bytes: Int)

    private val entries = LinkedHashMap<K, Entry<V>>()
    private val pending = LinkedHashMap<K, Deferred<V>>()
    private val waiters = mutableMapOf<Deferred<V>, Int>()
    private var bytes = 0
    private var epoch = 0L

    fun peek(key: K): V? {
        val entry = entries.remove(key) ?: return null
        if (clock() >= entry.expires) {
            bytes -= entry.bytes
            return null
        }
        entries[key] = entry
        return entry.value
    }

    suspend fun load(key: K, fetch: suspend () -> V): V {
        peek(key)?.let {
            return it
        }
        pending[key]?.let {
            return await(it)
        }
        // Rapid category taps must not accumulate unbounded detached JNI work.
        if (pending.size >= 8) {
            val abandoned =
                pending.entries.firstOrNull { (waiters[it.value] ?: 0) == 0 }
                    ?: throw IllegalStateException(
                        "Too many feed requests. Please try again shortly."
                    )
            pending.remove(abandoned.key)?.cancel()
        }
        val started = epoch
        val work =
            scope.async(start = CoroutineStart.LAZY) {
                try {
                    val value = fetch()
                    currentCoroutineContext().ensureActive()
                    if (started == epoch) {
                        val size = sizeOf(value)
                        if (size <= maxBytes) {
                            entries.remove(key)?.let { bytes -= it.bytes }
                            entries[key] = Entry(value, clock() + ttlMs, size)
                            bytes += size
                            while (entries.size > maxEntries || bytes > maxBytes) {
                                bytes -= entries.remove(entries.keys.first())!!.bytes
                            }
                        }
                    }
                    value
                } finally {
                    // Invalidation can already have installed a new fetch for this key.
                    if (pending[key] == currentCoroutineContext()[Job]) pending.remove(key)
                }
            }
        pending[key] = work
        return await(work)
    }

    private suspend fun await(work: Deferred<V>): V {
        waiters[work] = (waiters[work] ?: 0) + 1
        try {
            return work.await()
        } finally {
            val remaining = (waiters[work] ?: 1) - 1
            if (remaining == 0) waiters.remove(work) else waiters[work] = remaining
        }
    }

    fun invalidate(key: K) {
        entries.remove(key)?.let { bytes -= it.bytes }
        pending.remove(key)?.cancel()
    }

    fun clear() {
        epoch++
        val work = pending.values.toList()
        pending.clear()
        entries.clear()
        bytes = 0
        work.forEach { it.cancel() }
    }
}
