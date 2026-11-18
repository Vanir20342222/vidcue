package dev.ytui.android

/** Immutable page snapshots with incremental budget accounting, confined to the owner. */
internal class PageHistory<T>(
    private val maxEntries: Int,
    private val maxBytes: Long,
    private val sizeOf: (T) -> Long,
) {
    private val pages = ArrayDeque<Pair<T, Long>>()
    private var bytes = 0L

    fun addLast(value: T) {
        val size = sizeOf(value).coerceAtLeast(0)
        pages.addLast(value to size)
        bytes += size
        while (pages.size > maxEntries || bytes > maxBytes) bytes -= pages.removeFirst().second
    }

    fun removeLast(): T = pages.removeLast().let { bytes -= it.second; it.first }
    fun isEmpty() = pages.isEmpty()
    fun isNotEmpty() = pages.isNotEmpty()
    fun clear() { pages.clear(); bytes = 0 }
}
