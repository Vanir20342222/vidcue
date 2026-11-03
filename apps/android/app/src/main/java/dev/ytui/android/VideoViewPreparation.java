package dev.ytui.android;

import android.content.Context;
import android.content.ContextWrapper;
import android.os.Looper;
import android.os.MessageQueue;
import android.view.LayoutInflater;
import android.widget.FrameLayout;
import androidx.media3.ui.PlayerView;

/** One detached, unbound native view, owned only by its browsing activity. */
@androidx.annotation.OptIn(markerClass = androidx.media3.common.util.UnstableApi.class)
final class VideoViewPreparation {
  private final RustActivity owner;
  private MessageQueue.IdleHandler pending;
  private volatile PlayerView prepared;
  private boolean claimed;

  VideoViewPreparation(RustActivity owner) {
    this.owner = owner;
  }

  void schedule() {
    if (claimed || prepared != null || pending != null) return;
    pending =
        () -> {
          pending = null;
          if (owner.isFinishing()
              || owner.isDestroyed()
              || !owner
                  .getLifecycle()
                  .getCurrentState()
                  .isAtLeast(androidx.lifecycle.Lifecycle.State.RESUMED)) return false;
          try {
            // No window attachment, player binding, SurfaceTexture, decoder,
            // source preparation, notification, history or network request.
            prepared = inflate(owner);
          } catch (RuntimeException unavailable) {
            // Optional preparation; selection retains normal inflation/errors.
          }
          return false;
        };
    MessageQueue.IdleHandler idle = pending;
    // Let the first browsing frame draw before using an idle turn for inflation.
    owner
        .getWindow()
        .getDecorView()
        .postOnAnimation(
            () -> {
              if (pending == idle) Looper.myQueue().addIdleHandler(idle);
            });
  }

  void clear() {
    if (pending != null) {
      Looper.myQueue().removeIdleHandler(pending);
      pending = null;
    }
    prepared = null;
  }

  boolean ready() {
    return prepared != null;
  }

  PlayerView peek() {
    return prepared;
  }

  static PlayerView take(Context context) {
    Context candidate = context;
    while (candidate instanceof ContextWrapper) {
      if (candidate instanceof RustActivity) {
        VideoViewPreparation preparation = ((RustActivity) candidate).videoViews;
        preparation.claimed = true;
        PlayerView view = preparation.prepared;
        preparation.clear();
        if (view != null && view.getParent() == null) return view;
        break;
      }
      Context base = ((ContextWrapper) candidate).getBaseContext();
      if (base == candidate) break;
      candidate = base;
    }
    // Fullscreen/other activities must never borrow a view with a different owner.
    return inflate(context);
  }

  private static PlayerView inflate(Context context) {
    return (PlayerView)
        LayoutInflater.from(context).inflate(R.layout.ytui_player, new FrameLayout(context), false);
  }
}
