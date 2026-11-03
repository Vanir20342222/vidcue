package dev.ytui.android;

import android.os.Bundle;
import android.webkit.*;
import androidx.activity.ComponentActivity;

/** The only web content is the explicit account sign-in page, never the app UI. */
public final class AuthActivity extends ComponentActivity {
  WebView web;
  private String flow;
  private boolean loaded;
  private boolean wasFound;
  private final android.os.Handler poll = new android.os.Handler(android.os.Looper.getMainLooper());
  private final Runnable pollSession =
      new Runnable() {
        @Override
        public void run() {
          checkSession();
          if (!wasFound) poll.postDelayed(this, 1000);
        }
      };
  /**
   * Google's own sign-in page, returning to YouTube. Opening this directly skips the YouTube
   * mobile shell, which is a blank page until its overflow menu is used to start sign-in.
   */
  static final String SIGN_IN_URL =
      "https://accounts.google.com/ServiceLogin?service=youtube&continue="
          + "https%3A%2F%2Fwww.youtube.com%2Fsignin%3Faction_handle_signin%3Dtrue%26next%3D%252F";
  private static String activeFlow = "";
  private static String completedFlow = "";
  private static java.lang.ref.WeakReference<AuthActivity> active =
      new java.lang.ref.WeakReference<>(null);

  // Explicit account web flow only; no JavaScript host interface is exposed.
  @android.annotation.SuppressLint("SetJavaScriptEnabled")
  @Override
  public void onCreate(Bundle state) {
    super.onCreate(state);
    flow = state == null ? java.util.UUID.randomUUID().toString() : state.getString("auth-flow");
    active = new java.lang.ref.WeakReference<>(this);
    activeFlow = flow;
    if (state == null) {
      ComposeHost.resetAuth();
      AuthUi.reset(false);
      NativeBridge.beginAccountChange();
    } else AuthUi.showWeb(state.getBoolean("auth-web"));
    if (!AuthUi.motionKnown()) restoreMotionSetting();
    web = new WebView(this);
    web.getSettings().setJavaScriptEnabled(true);
    web.getSettings().setDomStorageEnabled(true);
    web.setWebViewClient(
        new WebViewClient() {
          @Override
          public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest request) {
            String host = request.getUrl().getHost();
            return host == null
                || !(host.equals("youtube.com")
                    || host.endsWith(".youtube.com")
                    || host.equals("google.com")
                    || host.endsWith(".google.com"));
          }

          @Override
          public void onPageStarted(WebView view, String url, android.graphics.Bitmap icon) {
            AuthUi.page(url);
          }

          @Override
          public void onPageFinished(WebView view, String url) {
            AuthUi.page(url);
            checkSession();
          }

          // YouTube's mobile page is a single-page app that can navigate (or stall blank)
          // without another page-finished callback, so history updates count too.
          @Override
          public void doUpdateVisitedHistory(WebView view, String url, boolean isReload) {
            AuthUi.page(url);
            checkSession();
          }
        });
    web.setWebChromeClient(
        new WebChromeClient() {
          @Override
          public void onProgressChanged(WebView view, int progress) {
            AuthUi.progress(progress);
            if (progress == 100) checkSession();
          }
        });
    ComposeHost.attachAuth(this, web);
    if (state != null) loaded = web.restoreState(state) != null;
    // Nothing committed before rotation, or the WebView package changed across process
    // death: the web stage would otherwise stay blank.
    if (state != null && AuthUi.showingWeb() && !loaded) load();
  }

  private void load() {
    loaded = true;
    if (BuildConfig.DEBUG && getIntent().getBooleanExtra("ytui.offline-auth", false))
      web.loadData(
          "<p>Offline sign-in UI fixture. No account is contacted.</p>", "text/html", "UTF-8");
    else web.loadUrl(SIGN_IN_URL);
  }

  /**
   * Only cookie names are checked; the user still confirms explicitly with the Use-session
   * action. Runs on every navigation signal and on a slow poll, because the final YouTube page
   * can stay blank and never report that it finished.
   */
  private void checkSession() {
    boolean found =
        AuthUi.looksSignedIn(CookieManager.getInstance().getCookie("https://www.youtube.com"));
    if (found && !wasFound && !ComposeHost.authBusy()) ComposeHost.authMessage("");
    wasFound = found;
    AuthUi.sessionFound(found);
  }

  /** Only when the process was recreated straight into sign-in; the app sets it otherwise. */
  private void restoreMotionSetting() {
    try {
      org.json.JSONObject data =
          new org.json.JSONObject(NativeBridge.request("{\"op\":\"settings\"}"))
              .optJSONObject("data");
      AuthUi.motion(data != null && data.optBoolean("reduced_motion"));
    } catch (Exception ignored) {
      // Keep the default; motion preference is cosmetic.
    }
  }

  /** Leaves the native account card for the provider's own sign-in page. */
  void beginWeb() {
    if (ComposeHost.authBusy()) return;
    ComposeHost.authMessage("");
    if (!loaded) load();
    AuthUi.showWeb(true);
    poll.removeCallbacks(pollSession);
    poll.post(pollSession);
  }

  /** Help/privacy/terms links open in the user's browser, never inside the sign-in view. */
  void openExternal(String url) {
    try {
      startActivity(
          new android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse(url)));
    } catch (android.content.ActivityNotFoundException ignored) {
      ComposeHost.authMessage("No browser is available to open " + url);
    }
  }

  void useSession() {
    if (ComposeHost.authBusy()) return;
    String header = CookieManager.getInstance().getCookie("https://www.youtube.com");
    if (header == null || header.isEmpty()) {
      ComposeHost.authMessage(
          "No signed-in YouTube session found. Complete sign-in or use cookie-file import.");
      return;
    }
    ComposeHost.setAuthBusy(true);
    long intent = NativeBridge.beginAccountChange();
    OfflineStore.IO.execute(
        () -> {
          try {
            org.json.JSONObject request =
                new org.json.JSONObject()
                    .put("op", "import_cookies")
                    .put("account_intent", intent)
                    .put("header", header);
            org.json.JSONObject response =
                new org.json.JSONObject(NativeBridge.request(request.toString()));
            MediaBridge.main.post(
                () -> {
                  if (!flow.equals(activeFlow)) return;
                  ComposeHost.setAuthBusy(false);
                  if (response.optBoolean("ok")) {
                    MediaBridge.event("account_updated", "");
                    clearAndFinish();
                  } else
                    ComposeHost.authMessage(
                        response.optString("error", "Could not import this session"));
                });
          } catch (Exception error) {
            MediaBridge.main.post(
                () -> {
                  if (!flow.equals(activeFlow)) return;
                  ComposeHost.setAuthBusy(false);
                  ComposeHost.authMessage(
                      "Could not import this session; use a cookie document instead");
                });
          }
        });
  }

  void cancelSession() {
    if (!ComposeHost.authBusy()) clearAndFinish();
  }

  private void clearAndFinish() {
    completedFlow = flow;
    CookieManager.getInstance()
        .removeAllCookies(
            ignored -> {
              CookieManager.getInstance().flush();
              AuthActivity target = active.get();
              if (target != null && flow.equals(target.flow)) target.finish();
            });
  }

  @Override
  protected void onResume() {
    super.onResume();
    if (flow.equals(completedFlow)) finish();
    poll.removeCallbacks(pollSession);
    if (loaded) poll.post(pollSession);
  }

  @Override
  protected void onPause() {
    poll.removeCallbacks(pollSession);
    super.onPause();
  }

  @Override
  protected void onSaveInstanceState(Bundle state) {
    state.putString("auth-flow", flow);
    state.putBoolean("auth-web", AuthUi.showingWeb());
    web.saveState(state);
    super.onSaveInstanceState(state);
  }

  @Override
  protected void onDestroy() {
    if (active.get() == this) active.clear();
    if (web != null) web.destroy();
    super.onDestroy();
  }
}
