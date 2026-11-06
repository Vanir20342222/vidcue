package dev.ytui.android;

import android.app.*;
import android.content.Intent;
import androidx.media3.exoplayer.offline.*;
import androidx.media3.exoplayer.scheduler.PlatformScheduler;
import androidx.media3.exoplayer.scheduler.Scheduler;
import java.util.ArrayList;
import java.util.List;

@androidx.annotation.OptIn(markerClass = androidx.media3.common.util.UnstableApi.class)
public final class OfflineService extends DownloadService {
  /**
   * JobScheduler id for restarting downloads once their network requirement is met. WorkManager
   * hands out JobScheduler ids counting up from 0 and only manages jobs of its own service, so a
   * fixed id near the top of the range cannot collide with it.
   */
  static final int RESTART_JOB_ID = 0x7663646c;

  public OfflineService() {
    super(NotificationIds.DOWNLOADS, 1000, "ytui-downloads", R.string.downloads_channel_name, 0);
  }

  @Override
  protected androidx.media3.exoplayer.offline.DownloadManager getDownloadManager() {
    return OfflineStore.manager();
  }

  /**
   * Media3 asks for this only below Android 12 (a foreground DownloadService cannot be restarted
   * from the background on 12+, so there it stays in the foreground while it waits). The job
   * restarts downloads stopped by a lost network without the app being opened.
   */
  @Override
  protected Scheduler getScheduler() {
    return new PlatformScheduler(this, RESTART_JOB_ID);
  }

  @Override
  protected Notification getForegroundNotification(List<Download> downloads, int requirements) {
    List<DownloadPolicy.Part> parts = new ArrayList<>(downloads.size());
    for (Download d : downloads)
      parts.add(
          new DownloadPolicy.Part(
              d.request.id,
              d.state,
              d.getBytesDownloaded(),
              d.contentLength,
              d.getPercentDownloaded()));
    DownloadPolicy.Summary summary = DownloadPolicy.summarize(parts);
    Notification.Builder builder =
        new Notification.Builder(this, "ytui-downloads")
            .setSmallIcon(dev.ytui.android.R.drawable.ic_vidcue_notification)
            .setContentTitle("Vidcue downloads")
            .setContentText(DownloadPolicy.notificationText(summary, requirements))
            .setContentIntent(
                PendingIntent.getActivity(
                    this,
                    NotificationIds.DOWNLOADS,
                    new Intent(this, RustActivity.class)
                        .setAction(Intent.ACTION_MAIN)
                        .addCategory(Intent.CATEGORY_LAUNCHER),
                    PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT))
            .setOnlyAlertOnce(true)
            .setShowWhen(false)
            .setOngoing(true);
    if (requirements == 0 && summary.downloading > 0)
      builder.setProgress(100, Math.max(0, summary.percent), summary.percent < 0);
    return builder.build();
  }

  /**
   * Android 15 limits dataSync foreground services to six hours a day and requires the service
   * to stop when that runs out. The download manager keeps its state; downloads continue the next
   * time the app or a download request starts the service.
   */
  @Override
  public void onTimeout(int startId, int fgsType) {
    stopSelf();
  }
}
