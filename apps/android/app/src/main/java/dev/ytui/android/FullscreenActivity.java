package dev.ytui.android;

import android.os.Bundle;
import androidx.activity.ComponentActivity;
import androidx.activity.OnBackPressedCallback;
import androidx.core.view.*;
import androidx.lifecycle.Lifecycle;
import androidx.media3.ui.PlayerView;

/** One shared decoder/session: fullscreen never extracts or prepares again. */
@androidx.annotation.OptIn(markerClass = androidx.media3.common.util.UnstableApi.class)
public final class FullscreenActivity extends ComponentActivity {
  static volatile boolean visible;
  private PlayerView view;
  private VisibleFrameTracker frames;

  @Override
  protected void onCreate(Bundle state) {
    super.onCreate(state);
    PlaybackService service = PlaybackService.current;
    if (service == null) {
      finish();
      return;
    }
    visible = true;
    getOnBackPressedDispatcher()
        .addCallback(
            this,
            new OnBackPressedCallback(true) {
              @Override
              public void handleOnBackPressed() {
                finish();
              }
            });
    ComposeHost.fullscreenChanged(true);
    WindowCompat.setDecorFitsSystemWindows(getWindow(), false);
    WindowInsetsControllerCompat bars =
        WindowCompat.getInsetsController(getWindow(), getWindow().getDecorView());
    bars.hide(WindowInsetsCompat.Type.systemBars());
    bars.setSystemBarsBehavior(WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
    getWindow().addFlags(android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
    view = (PlayerView) getLayoutInflater().inflate(R.layout.ytui_player, null);
    view.setBackgroundColor(android.graphics.Color.BLACK);
    view.setUseController(true);
    PlayerChrome.apply(view);
    android.view.View collapse = view.findViewById(R.id.ytui_collapse);
    collapse.setOnClickListener(button -> finish());
    collapse.setContentDescription(getString(R.string.ytui_player_exit_fullscreen));
    view.setControllerAnimationEnabled(
        !getIntent().getBooleanExtra("reduced_motion", false)
            && android.animation.ValueAnimator.areAnimatorsEnabled());
    // Captions use ytui's own on/off button instead of Media3's track list.
    PlayerChrome.bindCaptions(view);
    // Queue modes live in the app's player; the fullscreen chrome is for watching.
    view.setShowShuffleButton(false);
    view.setFullscreenButtonState(true);
    view.setFullscreenButtonClickListener(
        fullscreen -> {
          if (!fullscreen) finish();
        });
    frames = new VisibleFrameTracker(view);
    VideoGestures.attach(view);
    PlayerChrome.trackSponsorMarks(view);
    // The options layer sits above the video in this same window: every playback
    // option is reachable without leaving fullscreen or revealing system bars.
    android.widget.FrameLayout root = new android.widget.FrameLayout(this);
    root.setBackgroundColor(android.graphics.Color.BLACK);
    root.addView(view, matchParent());
    setContentView(root);
    root.addView(FullscreenOptions.attach(this, view), matchParent());
    ViewCompat.setOnApplyWindowInsetsListener(
        root,
        (target, insets) -> {
          fitControls(insets.getInsets(WindowInsetsCompat.Type.displayCutout()));
          return insets;
        });
  }

  private static android.widget.FrameLayout.LayoutParams matchParent() {
    return new android.widget.FrameLayout.LayoutParams(
        android.view.ViewGroup.LayoutParams.MATCH_PARENT,
        android.view.ViewGroup.LayoutParams.MATCH_PARENT);
  }

  /** Keep controls clear of cutouts and screen edges; scrims and video stay edge to edge. */
  private void fitControls(androidx.core.graphics.Insets cutout) {
    int edge = Math.round(16 * getResources().getDisplayMetrics().density);
    int[] ids = {
      R.id.ytui_top_bar,
      androidx.media3.ui.R.id.exo_bottom_bar,
      androidx.media3.ui.R.id.exo_progress,
      androidx.media3.ui.R.id.exo_minimal_controls
    };
    for (int id : ids) {
      android.view.View child = view.findViewById(id);
      if (child == null
          || !(child.getLayoutParams() instanceof android.view.ViewGroup.MarginLayoutParams))
        continue;
      android.view.ViewGroup.MarginLayoutParams params =
          (android.view.ViewGroup.MarginLayoutParams) child.getLayoutParams();
      params.leftMargin = Math.max(edge, cutout.left);
      params.rightMargin = Math.max(edge, cutout.right);
      if (id == R.id.ytui_top_bar) params.topMargin = cutout.top;
      child.setLayoutParams(params);
    }
  }

  @Override
  protected void onResume() {
    super.onResume();
    visible = true;
    MediaBridge.uiVisible = true;
    PlaybackService service = PlaybackService.current;
    if (view != null && service != null) frames.bind(service.controlsPlayer);
    MediaBridge.notifyUi();
  }

  @Override
  protected void onPause() {
    if (view != null) frames.bind(null);
    super.onPause();
  }

  @Override
  protected void onStop() {
    visible = false;
    RustActivity activity = RustActivity.current();
    if (activity == null
        || !activity.getLifecycle().getCurrentState().isAtLeast(Lifecycle.State.STARTED)) {
      MediaBridge.uiVisible = false;
      if (!isChangingConfigurations() && PlaybackService.current != null)
        PlaybackService.current.pauseForBackground();
    }
    super.onStop();
  }

  @Override
  protected void onDestroy() {
    visible = false;
    if (view != null) frames.bind(null);
    ComposeHost.fullscreenChanged(false);
    super.onDestroy();
  }
}
