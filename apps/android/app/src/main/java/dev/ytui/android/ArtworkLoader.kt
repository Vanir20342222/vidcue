package dev.ytui.android

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.async
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withPermit

/**
 * Shares fetch/decode work across cards. Leaving a card drops queued work, but a decode already
 * running can finish into the cache: blocking JNI cannot be cancelled halfway through anyway. Only
 * [parallelism] loads can run, including loads whose original card has left the screen. [cached]
 * and [store] must be thread-safe and own the bounded memory cache.
 */
internal class ArtworkLoader<K, V : Any>(
    private val scope: CoroutineScope,
    private val cached: (K) -> V?,
    private val store: (K, V) -> Unit,
    parallelism: Int = 4,
) {
    private class Flight<V>(var waiters: Int = 0, var started: Boolean = false) {
        lateinit var work: Deferred<V?>
    }

    private val lock = Any()
    private val slots = Semaphore(parallelism)
    private val pending = mutableMapOf<K, Flight<V>>()

    suspend fun load(key: K, fetch: suspend () -> V?): V? {
        val flight =
            synchronized(lock) {
                cached(key)?.let {
                    return it
                }
                val flight =
                    pending[key]
                        ?: run {
                            Flight<V>().also { flight ->
                                flight.work =
                                    scope.async(start = CoroutineStart.LAZY) {
                                        slots.withPermit {
                                            synchronized(lock) { flight.started = true }
                                            currentCoroutineContext().ensureActive()
                                            cached(key) ?: fetch()?.also { store(key, it) }
                                        }
                                    }
                                pending[key] = flight
                                flight.work.invokeOnCompletion {
                                    synchronized(lock) {
                                        if (pending[key] === flight) pending.remove(key)
                                    }
                                }
                            }
                        }
                flight.waiters++
                flight
            }
        try {
            return flight.work.await()
        } finally {
            synchronized(lock) {
                flight.waiters--
                if (flight.waiters == 0 && !flight.started) {
                    if (pending[key] === flight) pending.remove(key)
                    flight.work.cancel()
                }
            }
        }
    }
}
