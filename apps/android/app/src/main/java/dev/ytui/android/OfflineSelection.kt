package dev.ytui.android

import org.json.JSONObject

/** Keep library order and the exact saved copy while allowing one entry per media URL. */
internal fun offlinePlaylistMembers(downloads: List<JSONObject>): List<JSONObject> =
    downloads.map { download ->
        download.getJSONObject("media").copyJson().put("offline_id", download.getString("id"))
    }.distinctBy { it.optString("url") }

internal fun mergeOfflinePlaylistMembers(
    members: List<JSONObject>,
    downloads: List<JSONObject>,
): List<JSONObject> {
    val urls = members.map { it.optString("url") }.toSet()
    return members + offlinePlaylistMembers(downloads).filter { it.optString("url") !in urls }
}
