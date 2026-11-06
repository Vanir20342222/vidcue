package dev.ytui.android

import kotlinx.coroutines.*
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withPermit

/** Main-dispatcher confined, like FeedPageCache. Limits span cancelled/replaced batches. */
internal class FeedPagePreloader<K, V>(
    private val cache: FeedPageCache<K, V>,
    private val concurrency: Int = 2,
) {
    private val slots = Semaphore(concurrency)

    suspend fun <T> prepare(
        targets: List<T>,
        keyOf: (T) -> K,
        allowed: () -> Boolean,
        fetch: suspend (T) -> V,
    ) = coroutineScope {
        var stopped = false
        val remaining = targets.iterator()
        repeat(concurrency) {
            launch {
                while (remaining.hasNext() && !stopped && allowed()) {
                    val target = remaining.next()
                    try {
                        cache.load(keyOf(target)) {
                            // Keep the permit with the detached fetch when a tap cancels
                            // this batch and starts another. The tap can join that fetch.
                            slots.withPermit {
                                if (stopped || !allowed()) throw CancellationException()
                                fetch(target)
                            }
                        }
                    } catch (cancelled: CancellationException) {
                        throw cancelled
                    } catch (_: Exception) {
                        // Respect failures/cooldowns without retries or visible errors.
                        stopped = true
                    }
                }
            }
        }
    }
}
