package dev.ytui.android

// Wording and arithmetic for the Downloads page. Nothing here touches Compose or Android, so
// the rules for sizes, speed, time left and the status summary are testable on the JVM.

/** Coarse look of a download: drives the status colour and which numbers are worth showing. */
internal enum class TransferTone {
    /** Bytes are arriving right now. */
    Active,
    /** Queued, restarting or waiting for the other half of a video; nothing is arriving yet. */
    Waiting,
    Paused,
    Failed,
}

/** Media3 download states, plus the audio-pairing states `OfflineStore` adds. */
internal fun transferTone(state: String): TransferTone =
    when {
        state.contains("failed", ignoreCase = true) -> TransferTone.Failed
        state.equals("Paused", ignoreCase = true) -> TransferTone.Paused
        state.equals("Downloading", ignoreCase = true) -> TransferTone.Active
        else -> TransferTone.Waiting
    }

internal fun byteLabel(bytes: Long): String =
    when {
        bytes >= 1L shl 30 -> "%.1f GB".format(bytes / (1L shl 30).toDouble())
        bytes >= 1L shl 20 -> "%.0f MB".format(bytes / (1L shl 20).toDouble())
        else -> "%.0f KB".format(bytes / 1024.0)
    }

/** "12 MB of 1.2 GB"; just "12 MB" while the size is unknown; null before anything arrived. */
internal fun downloadedLabel(bytes: Long, total: Long): String? =
    when {
        total > 0 && bytes < total -> "${byteLabel(bytes.coerceAtLeast(0))} of ${byteLabel(total)}"
        bytes > 0 -> byteLabel(bytes)
        else -> null
    }

internal fun percentLabel(progress: Float): String =
    "${(progress.coerceIn(0f, 1f) * 100).toInt()}%"

internal fun speedLabel(bytesPerSecond: Double): String? =
    if (bytesPerSecond > 0.0) "${byteLabel(bytesPerSecond.toLong())}/s" else null

/** Seconds until the transfer ends at this speed; null when either number is not yet honest. */
internal fun secondsLeft(bytes: Long, total: Long, bytesPerSecond: Double): Long? =
    if (bytesPerSecond <= 0.0 || total <= bytes) null
    else ((total - bytes) / bytesPerSecond).toLong().coerceAtLeast(1)

/** "45 s left", "3 min left", "1 h 20 min left". */
internal fun timeLeftLabel(seconds: Long): String =
    when {
        seconds >= 3600 -> "${seconds / 3600} h ${seconds / 60 % 60} min left"
        seconds >= 60 -> "${seconds / 60} min left"
        else -> "${seconds.coerceAtLeast(1)} s left"
    }

/** What an offline copy is: "Video · 1080p · 1.2 GB" or "Audio · 54 MB". */
internal fun offlineDetails(audio: Boolean, quality: String, bytes: Long): String =
    listOfNotNull(
            if (audio) "Audio" else "Video",
            when {
                audio -> null
                quality.equals("best", ignoreCase = true) -> "Best quality"
                quality.isNotEmpty() && quality.all { it.isDigit() } -> "${quality}p"
                else -> null
            },
            if (bytes > 0) byteLabel(bytes) else null,
        )
        .joinToString(" · ")

/**
 * One line that says what the Transfers list holds, e.g. "2 downloading · 1 paused".
 * Order is fixed so the line does not reshuffle while counts change.
 */
internal fun transferSummary(tones: List<TransferTone>): String =
    listOf(
            TransferTone.Active to "downloading",
            TransferTone.Waiting to "waiting",
            TransferTone.Paused to "paused",
            TransferTone.Failed to "failed",
        )
        .mapNotNull { (tone, word) -> tones.count { it == tone }.takeIf { it > 0 }?.let { "$it $word" } }
        .joinToString(" · ")
