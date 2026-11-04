package dev.ytui.android

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInstaller
import android.os.Build

/**
 * Receives Android's result for an update session started by [AppUpdates]. Not exported:
 * only the system, through the session's PendingIntent, can reach it.
 */
class AppUpdateReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        when (val status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)) {
            PackageInstaller.STATUS_PENDING_USER_ACTION -> {
                // Android's own "Do you want to update this app?" screen.
                val confirm =
                    if (Build.VERSION.SDK_INT >= 33)
                        intent.getParcelableExtra(Intent.EXTRA_INTENT, Intent::class.java)
                    else @Suppress("DEPRECATION") intent.getParcelableExtra(Intent.EXTRA_INTENT)
                if (confirm == null) {
                    AppUpdates.installFinished("Android did not show the installation screen.")
                    return
                }
                runCatching { context.startActivity(confirm.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
                    .onFailure { AppUpdates.installFinished("Android did not show the installation screen.") }
            }
            // On success Android restarts Vidcue as the new version.
            PackageInstaller.STATUS_SUCCESS -> AppUpdates.installFinished(null)
            PackageInstaller.STATUS_FAILURE_ABORTED -> AppUpdates.installFinished("Update cancelled.")
            PackageInstaller.STATUS_FAILURE_CONFLICT,
            PackageInstaller.STATUS_FAILURE_INCOMPATIBLE ->
                AppUpdates.installFinished(
                    "Android refused the update: it is not signed with the key of the installed app."
                )
            PackageInstaller.STATUS_FAILURE_STORAGE ->
                AppUpdates.installFinished("Not enough storage to install the update.")
            else ->
                AppUpdates.installFinished(
                    intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE)
                        ?: "The update could not be installed ($status)."
                )
        }
    }
}
