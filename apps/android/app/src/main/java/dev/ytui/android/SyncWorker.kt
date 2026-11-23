package dev.ytui.android

import android.content.Context
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequest
import androidx.work.WorkManager
import androidx.work.Worker
import androidx.work.WorkerParameters
import java.util.concurrent.TimeUnit
import org.json.JSONObject

/**
 * Background sync while the app is closed: every 15 minutes on any network, one `sync_now`
 * through the engine (push the outbox, pull changes). Scheduled while signed in, cancelled on
 * sign-out; a run that finds no session cancels the schedule itself.
 */
internal class SyncWorker(context: Context, params: WorkerParameters) : Worker(context, params) {
    override fun doWork(): Result {
        if (!awaitEngine(ENGINE_WAIT_MS, { EngineStartup.current.isReady() }, { EngineStartup.current.error() != null }))
            return if (EngineStartup.current.error() != null) Result.failure() else Result.retry()
        return try {
            val status = request("sync_status")
            if (status?.optBoolean("signed_in") != true) {
                cancel(applicationContext)
                return Result.success()
            }
            if (!isStopped) request("sync_now")
            Result.success()
        } catch (failure: Exception) {
            // The engine keeps its own backoff; the next periodic run tries again.
            Result.success()
        }
    }

    private fun request(op: String): JSONObject? {
        val reply = JSONObject(NativeBridge.request(JSONObject().put("op", op).toString()))
        return if (reply.optBoolean("ok")) reply.optJSONObject("data") else null
    }

    companion object {
        const val WORK = "vidcue-sync"
        const val ENGINE_WAIT_MS = 20_000L

        fun schedule(context: Context) {
            val work =
                PeriodicWorkRequest.Builder(SyncWorker::class.java, 15, TimeUnit.MINUTES)
                    .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
                    .addTag(WORK)
                    .build()
            WorkManager.getInstance(context).enqueueUniquePeriodicWork(WORK, ExistingPeriodicWorkPolicy.KEEP, work)
        }

        fun cancel(context: Context) {
            WorkManager.getInstance(context).cancelUniqueWork(WORK)
        }

        /** Waits up to [timeoutMs] for the engine; false when it failed or is still starting. */
        internal fun awaitEngine(
            timeoutMs: Long,
            ready: () -> Boolean,
            failed: () -> Boolean,
            now: () -> Long = System::currentTimeMillis,
            sleep: (Long) -> Unit = Thread::sleep,
        ): Boolean {
            val deadline = now() + timeoutMs
            while (true) {
                if (ready()) return true
                if (failed() || now() >= deadline) return false
                sleep(100)
            }
        }
    }
}
