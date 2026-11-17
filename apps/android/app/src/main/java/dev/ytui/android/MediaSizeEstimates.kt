package dev.ytui.android

import org.json.JSONObject

/**
 * Media JSON is immutable after publication: edits must use copyJson, which gives a new identity.
 * Weak keys let cached pages reuse their off-main estimates without retaining evicted pages.
 */
internal object MediaSizeEstimates {
    private val sizes = java.util.WeakHashMap<JSONObject, Int>()

    fun bytes(value: JSONObject): Int {
        synchronized(sizes) { sizes[value] }?.let { return it }
        val bytes = value.toString().length * 2
        synchronized(sizes) { sizes[value] = bytes }
        return bytes
    }
}
