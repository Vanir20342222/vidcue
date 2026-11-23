//! Server-rendered pages: completing an invite or a reset link in a browser.

pub fn query_escape(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Builds a page response with a CSP that only allows the page's own inline script
/// (pinned by hash), no other origins, no framing, no forms and no base URL changes.
pub fn page_response(html: String) -> axum::response::Response {
    use axum::http::header;
    use axum::response::IntoResponse;
    let csp = format!(
        "default-src 'none'; script-src '{}'; style-src 'unsafe-inline'; img-src data:; \
         connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        script_hash()
    );
    (
        [
            (header::CACHE_CONTROL, "no-store".to_owned()),
            (header::REFERRER_POLICY, "no-referrer".to_owned()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
            (header::CONTENT_SECURITY_POLICY, csp),
        ],
        axum::response::Html(html),
    )
        .into_response()
}

fn inline_script() -> &'static str {
    let start = PAGE.find("<script>").expect("page has a script") + "<script>".len();
    let end = PAGE.find("</script>").expect("page has a script");
    &PAGE[start..end]
}

/// `sha256-…` source expression of the inline script.
pub fn script_hash() -> &'static str {
    static HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HASH.get_or_init(|| {
        use sha2::Digest;
        let digest = sha2::Sha256::digest(inline_script().as_bytes());
        format!("sha256-{}", data_encoding::BASE64.encode(&digest))
    })
}

/// Invite and reset tokens travel in the URL fragment (`/join#vci_…`), which browsers
/// never send to the server, so they stay out of proxy logs. The page only differs by
/// mode; the token is read by the script.
fn page(mode: &str) -> String {
    PAGE.replace("{{MODE}}", mode)
}

pub fn join_page() -> String {
    page("join")
}

pub fn reset_page() -> String {
    page("reset")
}

const PAGE: &str = r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>Vidcue account</title>
<style>
:root { --bg:#0f0f12; --card:#1a1a20; --text:#f2f2f5; --muted:#a0a0ab; --accent:#ff3d5a; --border:#2c2c35; --error:#ff7a8a; }
@media (prefers-color-scheme: light) { :root { --bg:#f6f6f8; --card:#fff; --text:#16161a; --muted:#5d5d68; --border:#dcdce3; --error:#c0182f; } }
* { box-sizing:border-box; }
body { margin:0; min-height:100vh; display:grid; place-items:center; background:var(--bg); color:var(--text);
       font:16px/1.5 system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; padding:16px; }
main { width:100%; max-width:380px; background:var(--card); border:1px solid var(--border); border-radius:20px; padding:28px 24px; }
h1 { margin:0 0 4px; font-size:24px; }
p { margin:0 0 20px; color:var(--muted); }
label { display:block; font-size:14px; color:var(--muted); margin:14px 0 6px; }
input { width:100%; padding:12px 14px; border-radius:12px; border:1px solid var(--border); background:transparent; color:var(--text); font:inherit; }
input:focus { outline:2px solid var(--accent); border-color:transparent; }
button { width:100%; margin-top:22px; padding:13px; border:0; border-radius:999px; background:var(--accent); color:#fff; font:600 16px system-ui, sans-serif; cursor:pointer; }
button:disabled { opacity:.6; }
.error { color:var(--error); min-height:1.5em; margin:12px 0 0; font-size:14px; }
.qr { background:#fff; border-radius:14px; padding:8px; display:flex; justify-content:center; margin:0 auto 12px; width:fit-content; }
.qr svg { display:block; width:200px; height:200px; }
code { display:block; word-break:break-all; font-size:13px; color:var(--muted); text-align:center; }
ol.codes { columns:2; margin:0 0 16px; padding-left:22px; font:15px/1.9 ui-monospace, monospace; }
.hidden { display:none; }
</style>
</head>
<body>
<main data-mode="{{MODE}}">
  <section id="account">
    <h1 id="title">Join Vidcue</h1>
    <p id="lead">Create your account.</p>
    <form id="account-form">
      <div id="username-field">
      <label for="username">Username</label>
      <input id="username" autocomplete="username" autocapitalize="none" spellcheck="false" minlength="3" maxlength="32">
      </div>
      <label for="password">Password</label>
      <input id="password" type="password" autocomplete="new-password" required minlength="10">
      <label for="password2">Repeat password</label>
      <input id="password2" type="password" autocomplete="new-password" required minlength="10">
      <button type="submit">Continue</button>
      <div class="error" role="alert"></div>
    </form>
  </section>
  <section id="totp" class="hidden">
    <h1>Authenticator</h1>
    <p>Scan with your authenticator app, then enter the 6-digit code.</p>
    <div class="qr" id="qr"></div>
    <code id="secret"></code>
    <form id="totp-form">
      <label for="code">Code</label>
      <input id="code" inputmode="numeric" autocomplete="one-time-code" pattern="[0-9 ]{6,7}" required>
      <button type="submit">Finish</button>
      <div class="error" role="alert"></div>
    </form>
  </section>
  <section id="done" class="hidden">
    <h1>You're in</h1>
    <p id="done-lead">Sign in from the Vidcue app with your username, password and authenticator code.</p>
    <p>Recovery codes. Each works once instead of an authenticator code. Store them somewhere safe; they are not shown again.</p>
    <ol class="codes" id="codes"></ol>
  </section>
</main>
<script>
(() => {
  const main = document.querySelector("main");
  const token = decodeURIComponent(location.hash.slice(1)), reset = main.dataset.mode === "reset";
  // Drop the token from the address bar and history once read.
  history.replaceState(null, "", location.pathname);
  let signupId = null;
  if (reset) {
    document.getElementById("title").textContent = "Reset account";
    document.getElementById("lead").textContent = "Choose a new password and set up your authenticator again. All devices will be signed out.";
    document.getElementById("username-field").classList.add("hidden");
    document.getElementById("done-lead").textContent = "Your account is reset. Sign in again on your devices.";
  } else {
    document.getElementById("username").required = true;
  }
  const show = id => document.querySelectorAll("section").forEach(s => s.classList.toggle("hidden", s.id !== id));
  async function call(path, body) {
    const res = await fetch(path, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) });
    const data = await res.json().catch(() => ({}));
    if (!res.ok) throw new Error(data.error || "Something went wrong");
    return data;
  }
  function form(id, run) {
    const el = document.getElementById(id), err = el.querySelector(".error"), btn = el.querySelector("button");
    el.addEventListener("submit", async e => {
      e.preventDefault(); err.textContent = ""; btn.disabled = true;
      try { await run(); } catch (x) { err.textContent = x.message; } finally { btn.disabled = false; }
    });
  }
  form("account-form", async () => {
    const pw = document.getElementById("password").value;
    if (pw !== document.getElementById("password2").value) throw new Error("Passwords do not match");
    const data = reset
      ? await call("/api/reset", { token, password: pw })
      : await call("/api/signup", { invite: token, username: document.getElementById("username").value, password: pw });
    signupId = data.signup_id;
    document.getElementById("qr").innerHTML = data.totp_qr_svg;
    document.getElementById("secret").textContent = data.totp_secret;
    show("totp");
    document.getElementById("code").focus();
  });
  form("totp-form", async () => {
    const code = document.getElementById("code").value;
    const data = reset
      ? await call("/api/reset/confirm", { token, code })
      : await call("/api/signup/confirm", { invite: token, signup_id: signupId, code });
    const list = document.getElementById("codes");
    list.replaceChildren(...(data.recovery_codes || []).map(c => Object.assign(document.createElement("li"), { textContent: c })));
    show("done");
  });
})();
</script>
</body>
</html>
"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_carry_no_token_and_one_hashed_script() {
        let (join, reset) = (join_page(), reset_page());
        assert!(join.contains("data-mode=\"join\"") && reset.contains("data-mode=\"reset\""));
        assert!(!join.contains("{{") && !reset.contains("{{"));
        assert_eq!(join.matches("<script>").count(), 1);
        assert!(inline_script().contains("location.hash"));
        assert!(script_hash().starts_with("sha256-") && script_hash().len() == 7 + 44);
        let res = page_response(join);
        let csp = res.headers()["content-security-policy"].to_str().unwrap();
        assert!(csp.contains(script_hash()) && !csp.contains("script-src 'unsafe-inline'"));
        assert!(csp.contains("form-action 'none'") && csp.contains("base-uri 'none'"));
    }

    #[test]
    fn escapes_query_values() {
        assert_eq!(query_escape("https://a.b/c d"), "https%3A%2F%2Fa.b%2Fc%20d");
    }
}
