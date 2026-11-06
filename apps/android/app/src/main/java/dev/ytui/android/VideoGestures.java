package dev.ytui.android;

import android.app.Activity;
import android.content.Context;
import android.content.ContextWrapper;
import android.media.AudioManager;
import android.view.GestureDetector;
import android.view.Gravity;
import android.view.HapticFeedbackConstants;
import android.view.MotionEvent;
import android.view.ScaleGestureDetector;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowManager;
import android.widget.FrameLayout;
import android.widget.TextView;
import androidx.media3.ui.AspectRatioFrameLayout;
import androidx.media3.ui.PlayerView;
import org.json.JSONObject;

/**
 * Touch gestures on the native video surface. Native controls keep their own touch handling;
 * every gesture is behind the master "gestures" switch plus its own setting:
 *
 * <ul>
 *   <li>double-tap left/right third: seek back/forward; middle third: play/pause
 *   <li>press and hold: temporary fast playback until released
 *   <li>vertical drag, left half: screen brightness; right half: media volume
 *   <li>pinch: zoom to fill / fit the whole frame
 * </ul>
 */
@androidx.annotation.OptIn(markerClass = androidx.media3.common.util.UnstableApi.class)
final class VideoGestures {
  private enum Drag {
    NONE,
    BRIGHTNESS,
    VOLUME,
    /** Vertical swipe handed to the watch page (minimize down, fullscreen up). */
    PANEL,
    /** Downward swipe in the fullscreen activity: leave fullscreen. */
    EXIT
  }

  /** Receives vertical swipes on the inline player so the panel can follow the finger. */
  interface PanelSwipe {
    boolean enabled();

    /** Total vertical travel since the swipe began, in pixels (down is positive). */
    void onDrag(float dy);

    /** Release: final travel and vertical velocity (px/s, down positive). Zero on cancel. */
    void onEnd(float dy, float velocity);
  }

  static void setPanelSwipe(PlayerView view, PanelSwipe swipe) {
    view.setTag(R.id.ytui_panel_swipe, swipe);
  }

  private final PlayerView view;
  private final TextView badge;
  private final GestureDetector taps;
  private final ScaleGestureDetector pinch;
  private final int slop;
  private Drag drag = Drag.NONE;
  private boolean holding;
  private boolean scaled;
  private float downX, downY, startLevel, dragScale = 1f;
  private boolean brightnessChanged;
  // Raw (screen) coordinates: the view itself moves while the panel follows the finger.
  private float panelStartY, panelLastY, panelVelocity;
  private long panelLastTime;

  static void attach(PlayerView view) {
    new VideoGestures(view);
  }

  private VideoGestures(PlayerView view) {
    this.view = view;
    Context context = view.getContext();
    slop = android.view.ViewConfiguration.get(context).getScaledTouchSlop();
    badge = new TextView(context);
    badge.setTextColor(0xffffffff);
    badge.setTextSize(15);
    badge.setTypeface(android.graphics.Typeface.DEFAULT_BOLD);
    int padH = dp(16), padV = dp(8);
    badge.setPadding(padH, padV, padH, padV);
    badge.setCompoundDrawablePadding(dp(10));
    badge.setGravity(Gravity.CENTER_VERTICAL);
    android.graphics.drawable.GradientDrawable pill =
        new android.graphics.drawable.GradientDrawable();
    pill.setColor(0xb8000000);
    pill.setCornerRadius(dp(24));
    badge.setBackground(pill);
    badge.setAlpha(0f);
    badge.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
    FrameLayout.LayoutParams params =
        new FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT,
            ViewGroup.LayoutParams.WRAP_CONTENT,
            Gravity.CENTER_HORIZONTAL | Gravity.TOP);
    params.topMargin = dp(24);
    FrameLayout overlay = view.getOverlayFrameLayout();
    (overlay != null ? overlay : view).addView(badge, params);

    taps =
        new GestureDetector(
            context,
            new GestureDetector.SimpleOnGestureListener() {
              @Override
              public boolean onDown(MotionEvent event) {
                return true;
              }

              @Override
              public boolean onSingleTapConfirmed(MotionEvent event) {
                view.performClick();
                return true;
              }

              @Override
              public boolean onSingleTapUp(MotionEvent event) {
                // Without double-tap seeking, show controls immediately (no 300 ms wait).
                if (!MobileSettings.gesture("gesture_double_tap_seek")) view.performClick();
                return true;
              }

              @Override
              public boolean onDoubleTap(MotionEvent event) {
                if (!MobileSettings.gesture("gesture_double_tap_seek")) return false;
                float third = view.getWidth() / 3f;
                int seconds = MobileSettings.integer("seek_seconds");
                if (event.getX() < third) {
                  command("back");
                  show(R.drawable.ytui_gesture_seek_back, false, seconds + " s", true);
                } else if (event.getX() > third * 2) {
                  command("forward");
                  show(R.drawable.ytui_gesture_seek_forward, true, seconds + " s", true);
                } else {
                  command("pause");
                  PlaybackService service = PlaybackService.current;
                  boolean playing = service != null && service.player.getPlayWhenReady();
                  show(playing ? "Playing" : "Paused");
                }
                haptic(HapticFeedbackConstants.VIRTUAL_KEY);
                return true;
              }

              @Override
              public void onLongPress(MotionEvent event) {
                if (drag != Drag.NONE || scaled || !MobileSettings.gesture("gesture_hold_speed"))
                  return;
                holding = true;
                command(new JSONObject(), "speed_hold", true);
                float rate = MobileSettings.integer("hold_speed") / 100f;
                show(R.drawable.ytui_gesture_seek_forward, true, formatRate(rate) + "×", false);
                haptic(HapticFeedbackConstants.LONG_PRESS);
              }
            });
    pinch =
        new ScaleGestureDetector(
            context,
            new ScaleGestureDetector.SimpleOnScaleGestureListener() {
              @Override
              public boolean onScaleBegin(ScaleGestureDetector detector) {
                if (!MobileSettings.gesture("gesture_pinch_zoom")) return false;
                scaled = true;
                dragScale = 1f;
                return true;
              }

              @Override
              public boolean onScale(ScaleGestureDetector detector) {
                dragScale *= detector.getScaleFactor();
                return true;
              }

              @Override
              public void onScaleEnd(ScaleGestureDetector detector) {
                if (dragScale > 1.08f
                    && view.getResizeMode() != AspectRatioFrameLayout.RESIZE_MODE_ZOOM) {
                  view.setResizeMode(AspectRatioFrameLayout.RESIZE_MODE_ZOOM);
                  show("Zoomed to fill");
                  haptic(HapticFeedbackConstants.CONTEXT_CLICK);
                } else if (dragScale < 0.92f
                    && view.getResizeMode() != AspectRatioFrameLayout.RESIZE_MODE_FIT) {
                  view.setResizeMode(AspectRatioFrameLayout.RESIZE_MODE_FIT);
                  show("Original");
                  haptic(HapticFeedbackConstants.CONTEXT_CLICK);
                }
              }
            });
    view.setOnTouchListener((target, event) -> onTouch(event));
    view.addOnAttachStateChangeListener(
        new View.OnAttachStateChangeListener() {
          @Override
          public void onViewAttachedToWindow(View v) {}

          @Override
          public void onViewDetachedFromWindow(View v) {
            release();
            // Brightness only follows the video; leaving it restores the system level.
            if (brightnessChanged) setBrightness(WindowManager.LayoutParams.BRIGHTNESS_OVERRIDE_NONE);
          }
        });
  }

  private boolean onTouch(MotionEvent event) {
    pinch.onTouchEvent(event);
    switch (event.getActionMasked()) {
      case MotionEvent.ACTION_DOWN:
        downX = event.getX();
        downY = event.getY();
        drag = Drag.NONE;
        scaled = false;
        break;
      case MotionEvent.ACTION_POINTER_DOWN:
        // Second finger: a pinch, never a tap, hold or level drag.
        drag = Drag.NONE;
        cancelTaps(event);
        return true;
      case MotionEvent.ACTION_MOVE:
        if (scaled || event.getPointerCount() > 1) return true;
        if (drag == Drag.NONE && !holding) {
          float dy = event.getY() - downY, dx = event.getX() - downX;
          if (Math.abs(dy) > slop * 2 && Math.abs(dy) > Math.abs(dx) * 1.5f) {
            PanelSwipe panel = (PanelSwipe) view.getTag(R.id.ytui_panel_swipe);
            boolean swipes = MobileSettings.gesture("gesture_swipe_dismiss");
            boolean middle = downX > view.getWidth() / 3f && downX < view.getWidth() * 2f / 3f;
            if (panel != null && panel.enabled() && swipes) {
              // Watch page: like YouTube, a vertical swipe minimizes or goes fullscreen.
              drag = Drag.PANEL;
              panelStartY = panelLastY = event.getRawY();
              panelLastTime = event.getEventTime();
              panelVelocity = 0f;
            } else if (panel == null && swipes && middle && dy > 0
                && activity() instanceof FullscreenActivity) {
              drag = Drag.EXIT;
              panelStartY = event.getRawY();
            } else if (MobileSettings.gesture("gesture_video_adjust")) {
              drag = downX < view.getWidth() / 2f ? Drag.BRIGHTNESS : Drag.VOLUME;
              startLevel = drag == Drag.BRIGHTNESS ? brightness() : volume();
              downY = event.getY();
            }
            if (drag != Drag.NONE) {
              if (view.getParent() != null)
                view.getParent().requestDisallowInterceptTouchEvent(true);
              cancelTaps(event);
            }
          }
        }
        if (drag == Drag.PANEL) {
          PanelSwipe panel = (PanelSwipe) view.getTag(R.id.ytui_panel_swipe);
          float y = event.getRawY();
          long dt = event.getEventTime() - panelLastTime;
          if (dt > 0) panelVelocity = 0.6f * panelVelocity + 0.4f * ((y - panelLastY) * 1000f / dt);
          panelLastY = y;
          panelLastTime = event.getEventTime();
          if (panel != null) panel.onDrag(y - panelStartY);
          return true;
        }
        if (drag == Drag.EXIT) return true;
        if (drag != Drag.NONE) {
          // A full-height drag sweeps the whole range.
          float delta = (downY - event.getY()) / Math.max(1, view.getHeight());
          float level = Math.max(0f, Math.min(1f, startLevel + delta));
          if (drag == Drag.BRIGHTNESS) {
            setBrightness(Math.max(0.01f, level));
            int percent = Math.round(level * 100);
            show(
                percent < 50
                    ? R.drawable.ytui_gesture_brightness_low
                    : R.drawable.ytui_gesture_brightness_high,
                false,
                percent + "%",
                false);
          } else {
            setVolume(level);
            int percent = Math.round(level * 100);
            show(
                percent == 0
                    ? R.drawable.ytui_gesture_volume_mute
                    : percent < 50
                        ? R.drawable.ytui_gesture_volume_low
                        : R.drawable.ytui_gesture_volume_high,
                false,
                percent + "%",
                false);
          }
          return true;
        }
        break;
      case MotionEvent.ACTION_UP:
      case MotionEvent.ACTION_CANCEL:
        boolean consumed = drag != Drag.NONE || holding || scaled;
        if (drag == Drag.PANEL) {
          PanelSwipe panel = (PanelSwipe) view.getTag(R.id.ytui_panel_swipe);
          boolean cancelled = event.getActionMasked() == MotionEvent.ACTION_CANCEL;
          if (panel != null)
            panel.onEnd(
                cancelled ? 0f : event.getRawY() - panelStartY, cancelled ? 0f : panelVelocity);
        } else if (drag == Drag.EXIT
            && event.getActionMasked() == MotionEvent.ACTION_UP
            && event.getRawY() - panelStartY > dp(72)) {
          Activity activity = activity();
          if (activity != null) activity.finish();
        }
        if (drag != Drag.NONE) hide();
        drag = Drag.NONE;
        release();
        if (consumed) {
          cancelTaps(event);
          return true;
        }
        break;
      default:
        break;
    }
    if (scaled) return true;
    return taps.onTouchEvent(event);
  }

  private void cancelTaps(MotionEvent event) {
    MotionEvent cancel = MotionEvent.obtain(event);
    cancel.setAction(MotionEvent.ACTION_CANCEL);
    taps.onTouchEvent(cancel);
    cancel.recycle();
  }

  private void release() {
    if (!holding) return;
    holding = false;
    command(new JSONObject(), "speed_hold", false);
    hide();
  }

  private static void command(String op) {
    command(new JSONObject(), op, null);
  }

  private static void command(JSONObject value, String op, Boolean active) {
    PlaybackService service = PlaybackService.current;
    if (service == null) return;
    try {
      value.put("op", op);
      if (active != null) value.put("active", active);
      service.command(value);
    } catch (Exception ignored) {
    }
  }

  private Activity activity() {
    Context context = view.getContext();
    while (context instanceof ContextWrapper) {
      if (context instanceof Activity) return (Activity) context;
      context = ((ContextWrapper) context).getBaseContext();
    }
    return null;
  }

  private float brightness() {
    Activity activity = activity();
    if (activity != null) {
      float current = activity.getWindow().getAttributes().screenBrightness;
      if (current >= 0) return current;
    }
    try {
      return android.provider.Settings.System.getInt(
              view.getContext().getContentResolver(),
              android.provider.Settings.System.SCREEN_BRIGHTNESS)
          / 255f;
    } catch (Exception unknown) {
      return 0.5f;
    }
  }

  private void setBrightness(float value) {
    Activity activity = activity();
    if (activity == null) return;
    WindowManager.LayoutParams attributes = activity.getWindow().getAttributes();
    attributes.screenBrightness = value;
    activity.getWindow().setAttributes(attributes);
    brightnessChanged = value >= 0;
  }

  private AudioManager audio() {
    return (AudioManager) view.getContext().getSystemService(Context.AUDIO_SERVICE);
  }

  private float volume() {
    AudioManager audio = audio();
    int max = audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC);
    return max <= 0 ? 0f : audio.getStreamVolume(AudioManager.STREAM_MUSIC) / (float) max;
  }

  private void setVolume(float value) {
    AudioManager audio = audio();
    int max = audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC);
    int target = Math.round(value * max);
    if (target != audio.getStreamVolume(AudioManager.STREAM_MUSIC))
      try {
        audio.setStreamVolume(AudioManager.STREAM_MUSIC, target, 0);
      } catch (SecurityException doNotDisturb) {
        // Do Not Disturb can refuse volume changes; leave the level unchanged.
      }
  }

  private void haptic(int kind) {
    if (MobileSettings.enabled("haptics")) view.performHapticFeedback(kind);
  }

  private static String formatRate(float rate) {
    return rate == Math.round(rate) ? String.valueOf(Math.round(rate)) : String.valueOf(rate);
  }

  private void show(String text) {
    show(text, true);
  }

  private void show(String text, boolean autoHide) {
    show(0, false, text, autoHide);
  }

  /**
   * Brief on-video feedback; reduced/disabled animations jump straight to the end state. {@code
   * icon} is a white vector drawable (0 for none) drawn before the text, or after it when {@code
   * iconAfter} is set.
   */
  private void show(int icon, boolean iconAfter, String text, boolean autoHide) {
    badge.setText(text);
    badge.setCompoundDrawablesRelativeWithIntrinsicBounds(
        iconAfter ? 0 : icon, 0, iconAfter ? icon : 0, 0);
    badge.animate().cancel();
    boolean animate = android.animation.ValueAnimator.areAnimatorsEnabled();
    if (animate && badge.getAlpha() < 1f)
      badge.animate().alpha(1f).setDuration(120).start();
    else badge.setAlpha(1f);
    badge.removeCallbacks(hider);
    if (autoHide) badge.postDelayed(hider, 650);
  }

  private final Runnable hider = this::hide;

  private void hide() {
    badge.removeCallbacks(hider);
    badge.animate().cancel();
    if (android.animation.ValueAnimator.areAnimatorsEnabled())
      badge.animate().alpha(0f).setDuration(220).start();
    else badge.setAlpha(0f);
  }

  private int dp(int value) {
    return Math.round(value * view.getResources().getDisplayMetrics().density);
  }
}
