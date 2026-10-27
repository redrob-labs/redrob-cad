//! Connecting Redrob CAD to a Redrob workspace without anyone handling the key.
//!
//! The console implements RFC 8628's device authorization grant. The app asks it for a code under
//! the product id `cad`, the person approves that code on console.redrob.ai, and the app polls until
//! the console hands back a workspace API key. Drafting never needs this; the built-in agent does,
//! and receives the key as `REDROB_API_KEY`, the variable Redrob Code reads for its Redrob provider.
//!
//! The device code stays inside this module: it is never shown, logged or stored. A pending
//! connection lives in memory only. The key is stored in `account.json` in the app's config
//! directory, readable by this user alone (0600 on Unix), the same protection Redrob Code gives its
//! own `auth.json`. Nothing retries a refusal: the console never turns a refused code into a key.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use redrob_cad_ui_egui::account::{Account, AccountService, ConnectState, PendingConnect};
use serde_json::{Value, json};

pub const CONSOLE_API: &str = "https://console.redrob.ai/api/backend/v1";
/// Overrides the console base URL (tests, staging).
pub const CONSOLE_API_ENV: &str = "REDROB_CONSOLE_API";
/// Overrides where `account.json` lives.
pub const CONFIG_DIR_ENV: &str = "REDROB_CAD_CONFIG_DIR";
pub const PRODUCT: &str = "cad";

const FALLBACK_EXPIRES: Duration = Duration::from_secs(600);
const MAX_EXPIRES: Duration = Duration::from_secs(1800);
const FALLBACK_INTERVAL: Duration = Duration::from_secs(5);
/// RFC 8628 3.5: on `slow_down` the interval grows by at least five seconds.
const SLOW_DOWN: Duration = Duration::from_secs(5);
/// The console restarts its interval window on every poll; a second of headroom avoids `slow_down`.
const MARGIN: Duration = Duration::from_secs(1);
const MAX_UNREACHABLE: u32 = 5;

/// The console's answer to one POST: HTTP status and JSON body (`{}` when it had none).
pub trait Transport: Send + Sync {
    fn post(&self, url: &str, body: &Value) -> Result<(u16, Value), String>;
}

pub struct Https;

impl Transport for Https {
    fn post(&self, url: &str, body: &Value) -> Result<(u16, Value), String> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(20)))
            .http_status_as_error(false)
            .user_agent(concat!("redrob-cad/", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        let mut resp = agent.post(url).header("content-type", "application/json").send(body.to_string()).map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let text = resp.body_mut().read_to_string().unwrap_or_default();
        Ok((status, serde_json::from_str(&text).unwrap_or_else(|_| json!({}))))
    }
}

/// The connected workspace as stored. Only `api_key` is secret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stored {
    pub api_key: String,
    pub api_key_name: String,
    pub account_name: String,
    pub api_base_url: String,
}

impl Stored {
    fn to_json(&self) -> Value {
        json!({"apiKey": self.api_key, "apiKeyName": self.api_key_name, "accountName": self.account_name, "apiBaseUrl": self.api_base_url})
    }
    fn from_json(v: &Value) -> Option<Self> {
        let s = |k: &str| v[k].as_str().map(str::trim).unwrap_or("").to_string();
        let api_key = s("apiKey");
        (!api_key.is_empty()).then(|| Stored {
            api_key,
            api_key_name: s("apiKeyName"),
            account_name: s("accountName"),
            api_base_url: s("apiBaseUrl"),
        })
    }
}

/// Overrides the shared account file's directory (Redrob Launcher's `REDROB_SHARED_DIR`).
pub const SHARED_DIR_ENV: &str = "REDROB_SHARED_DIR";

/// Redrob Launcher's shared account file: the workspace the computer is signed in to, which every
/// Redrob app may read and only the launcher writes (redrob-launcher `docs/shared-account.md`).
/// Same shape as this app's own `account.json`.
pub fn shared_account_path() -> Option<PathBuf> {
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let dir = if let Some(d) = env(SHARED_DIR_ENV) {
        d
    } else if cfg!(windows) {
        env("APPDATA")?.join("Redrob").join("Shared")
    } else if cfg!(target_os = "macos") {
        env("HOME")?.join("Library/Application Support/ai.redrob.shared")
    } else {
        env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|h| h.join(".config")))?.join("redrob-shared")
    };
    Some(dir.join("account.json"))
}

/// The key the app uses now: its own when it has one, else the computer's shared one, read fresh
/// every time (the person may sign in or out in Redrob Launcher while this app is open).
#[derive(Clone)]
pub struct KeySource {
    own: Arc<Mutex<Option<Stored>>>,
    shared: Option<PathBuf>,
}

impl KeySource {
    /// The key and whether it is the shared one.
    pub fn current(&self) -> Option<(Stored, bool)> {
        if let Some(s) = self.own.lock().ok().and_then(|g| g.clone()) {
            return Some((s, false));
        }
        self.shared.as_deref().and_then(load).map(|s| (s, true))
    }
}

/// `account.json`'s directory for this platform.
pub fn config_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os(CONFIG_DIR_ENV).filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(windows) {
        env("APPDATA").map(|d| d.join("Redrob CAD"))
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library/Application Support/ai.redrob.cad"))
    } else {
        env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|h| h.join(".config"))).map(|d| d.join("redrob-cad"))
    }
}

pub fn load(path: &Path) -> Option<Stored> {
    let text = std::fs::read_to_string(path).ok()?;
    Stored::from_json(&serde_json::from_str(&text).ok()?)
}

/// Write the key readable by this user only: a 0600 temp file renamed over the old one.
pub fn save(path: &Path, s: &Stored) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    {
        use std::io::Write;
        let mut f = opts.open(&tmp)?;
        f.write_all(serde_json::to_string_pretty(&s.to_json()).unwrap_or_default().as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

/// How one poll ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Poll {
    Pending,
    SlowDown,
    Connected(Stored),
    Denied,
    Expired,
    Unreachable,
    Failed(String),
}

/// What the console said when asked for a code. `device_code` never leaves this module.
pub struct Started {
    device_code: String,
    pub user_code: String,
    pub url: String,
    pub expires_in: Duration,
    pub interval: Duration,
}

pub fn authorize(t: &dyn Transport, base: &str) -> Result<Started, String> {
    let (status, b) =
        t.post(&format!("{base}/device/authorize"), &json!({"product": PRODUCT})).map_err(|_| "Could not reach console.redrob.ai.".to_string())?;
    if !(200..300).contains(&status) {
        return Err("console.redrob.ai would not start the connection.".into());
    }
    let s = |k: &str| b[k].as_str().map(str::trim).unwrap_or("").to_string();
    let (device_code, user_code, uri) = (s("deviceCode"), s("userCode"), s("verificationUri"));
    if device_code.is_empty() || user_code.is_empty() || uri.is_empty() {
        return Err("console.redrob.ai did not return a usable code.".into());
    }
    let complete = s("verificationUriComplete");
    let secs = |k: &str| b[k].as_f64().filter(|v| v.is_finite() && *v > 0.0).map(Duration::from_secs_f64);
    Ok(Started {
        device_code,
        user_code,
        url: if complete.is_empty() { uri } else { complete },
        expires_in: secs("expiresIn").unwrap_or(FALLBACK_EXPIRES).min(MAX_EXPIRES),
        interval: secs("interval").unwrap_or(FALLBACK_INTERVAL).max(Duration::from_secs(1)),
    })
}

pub fn poll_once(t: &dyn Transport, base: &str, started: &Started) -> Poll {
    let Ok((status, b)) = t.post(&format!("{base}/device/token"), &json!({"deviceCode": started.device_code})) else { return Poll::Unreachable };
    if (200..300).contains(&status) {
        let mut v = b.clone();
        if v["apiBaseUrl"].as_str().is_none_or(str::is_empty) {
            v["apiBaseUrl"] = json!(base);
        }
        return Stored::from_json(&v).map(Poll::Connected).unwrap_or(Poll::Failed("missing_key".into()));
    }
    match b["error"].as_str().unwrap_or("") {
        "authorization_pending" => Poll::Pending,
        "slow_down" => Poll::SlowDown,
        "access_denied" => Poll::Denied,
        "expired_token" => Poll::Expired,
        "" => Poll::Failed(format!("http_{status}")),
        other => Poll::Failed(other.to_string()),
    }
}

struct Attempt {
    state: ConnectState,
    expires: Option<Instant>,
    cancelled: Arc<AtomicBool>,
}

/// The app's account: the stored key, and at most one connection attempt in the background.
pub struct DeviceAccount {
    path: Option<PathBuf>,
    /// Redrob Launcher's shared account file, the fallback when this app has no key of its own.
    shared: Option<PathBuf>,
    /// The shared key last seen, so a change there restarts the agent like a change here does.
    shared_seen: Mutex<Option<String>>,
    stored: Arc<Mutex<Option<Stored>>>,
    attempt: Arc<Mutex<Attempt>>,
    generation: Arc<AtomicU64>,
    transport: Arc<dyn Transport>,
    base: String,
    ctx: Option<egui::Context>,
}

impl DeviceAccount {
    pub fn new(path: Option<PathBuf>, shared: Option<PathBuf>, transport: Arc<dyn Transport>, base: String, ctx: Option<egui::Context>) -> Self {
        let stored = path.as_deref().and_then(load);
        let shared_seen = Mutex::new(shared.as_deref().and_then(load).map(|s| s.api_key));
        Self {
            path,
            shared,
            shared_seen,
            stored: Arc::new(Mutex::new(stored)),
            attempt: Arc::new(Mutex::new(Attempt { state: ConnectState::Idle, expires: None, cancelled: Arc::new(AtomicBool::new(false)) })),
            generation: Arc::new(AtomicU64::new(0)),
            transport,
            base: base.trim_end_matches('/').to_string(),
            ctx,
        }
    }

    /// The key the agent is started with, read at the moment it starts.
    pub fn key_source(&self) -> KeySource {
        KeySource { own: self.stored.clone(), shared: self.shared.clone() }
    }
}

fn plain(p: &Poll) -> String {
    match p {
        Poll::Denied => "The connection was declined on console.redrob.ai.".into(),
        Poll::Expired => "The code expired. Connect again for a new one.".into(),
        Poll::Unreachable => "Could not reach console.redrob.ai. Check the network and connect again.".into(),
        Poll::Failed(code) => format!("console.redrob.ai refused the connection ({code})."),
        _ => String::new(),
    }
}

impl AccountService for DeviceAccount {
    fn account(&self) -> Option<Account> {
        let (s, via_launcher) = self.key_source().current()?;
        Some(Account {
            account_name: if s.account_name.is_empty() { "Redrob workspace".into() } else { s.account_name },
            key_name: s.api_key_name,
            via_launcher,
        })
    }

    fn state(&mut self) -> ConnectState {
        let Ok(a) = self.attempt.lock() else { return ConnectState::Idle };
        match (&a.state, a.expires) {
            (ConnectState::Waiting(p), Some(exp)) => {
                ConnectState::Waiting(PendingConnect { seconds_left: exp.saturating_duration_since(Instant::now()).as_secs(), ..p.clone() })
            }
            (s, _) => s.clone(),
        }
    }

    fn connect(&mut self) {
        let cancelled = Arc::new(AtomicBool::new(false));
        if let Ok(mut a) = self.attempt.lock() {
            if matches!(a.state, ConnectState::Starting | ConnectState::Waiting(_)) {
                return;
            }
            *a = Attempt { state: ConnectState::Starting, expires: None, cancelled: cancelled.clone() };
        }
        let (attempt, stored, generation, transport, base, path, ctx) = (
            self.attempt.clone(),
            self.stored.clone(),
            self.generation.clone(),
            self.transport.clone(),
            self.base.clone(),
            self.path.clone(),
            self.ctx.clone(),
        );
        let cancelled2 = cancelled.clone();
        let set = move |state: ConnectState, expires: Option<Instant>| {
            if let Ok(mut a) = attempt.lock()
                && Arc::ptr_eq(&a.cancelled, &cancelled)
                && !cancelled.load(Ordering::SeqCst)
            {
                a.state = state;
                a.expires = expires;
            }
            if let Some(c) = &ctx {
                c.request_repaint();
            }
        };
        std::thread::spawn(move || {
            let started = match authorize(transport.as_ref(), &base) {
                Ok(s) => s,
                Err(e) => return set(ConnectState::Failed(e), None),
            };
            let deadline = Instant::now() + started.expires_in;
            let _ = open_url(&started.url);
            set(
                ConnectState::Waiting(PendingConnect {
                    user_code: started.user_code.clone(),
                    url: started.url.clone(),
                    seconds_left: started.expires_in.as_secs(),
                }),
                Some(deadline),
            );
            let mut interval = started.interval;
            let mut unreachable = 0;
            loop {
                std::thread::sleep(interval + MARGIN);
                if cancelled2.load(Ordering::SeqCst) {
                    return;
                }
                if Instant::now() >= deadline {
                    return set(ConnectState::Failed(plain(&Poll::Expired)), None);
                }
                match poll_once(transport.as_ref(), &base, &started) {
                    Poll::Pending => unreachable = 0,
                    Poll::SlowDown => interval += SLOW_DOWN,
                    Poll::Unreachable => {
                        unreachable += 1;
                        if unreachable >= MAX_UNREACHABLE {
                            return set(ConnectState::Failed(plain(&Poll::Unreachable)), None);
                        }
                    }
                    Poll::Connected(s) => {
                        if let Some(p) = &path
                            && let Err(e) = save(p, &s)
                        {
                            return set(ConnectState::Failed(format!("Connected, but the key could not be saved: {e}")), None);
                        }
                        if let Ok(mut st) = stored.lock() {
                            *st = Some(s);
                        }
                        generation.fetch_add(1, Ordering::SeqCst);
                        return set(ConnectState::Idle, None);
                    }
                    other => return set(ConnectState::Failed(plain(&other)), None),
                }
            }
        });
    }

    fn cancel(&mut self) {
        if let Ok(mut a) = self.attempt.lock() {
            a.cancelled.store(true, Ordering::SeqCst);
            *a = Attempt { state: ConnectState::Idle, expires: None, cancelled: Arc::new(AtomicBool::new(false)) };
        }
    }

    fn disconnect(&mut self) {
        // The key itself stays valid on the console; it is revoked there (Settings > API keys).
        if let Some(p) = &self.path {
            let _ = std::fs::remove_file(p);
        }
        if let Ok(mut st) = self.stored.lock() {
            *st = None;
        }
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    fn generation(&self) -> u64 {
        // Signing in or out in Redrob Launcher changes the shared key: count it like a change here.
        let now = self.shared.as_deref().and_then(load).map(|s| s.api_key);
        if let Ok(mut seen) = self.shared_seen.lock()
            && *seen != now
        {
            *seen = now;
            self.generation.fetch_add(1, Ordering::SeqCst);
        }
        self.generation.load(Ordering::SeqCst)
    }
}

/// Open the approval page in the default browser; the palette also shows it as a link.
fn open_url(url: &str) -> std::io::Result<()> {
    if !url.starts_with("https://") {
        return Ok(());
    }
    let mut cmd = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(windows) {
        let mut c = std::process::Command::new("rundll32");
        c.arg("url.dll,FileProtocolHandler");
        c
    } else {
        std::process::Command::new("xdg-open")
    };
    cmd.arg(url).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// Answers as the console would, in order, and records what was asked.
    struct Script(Mutex<(VecDeque<Result<(u16, Value), String>>, Vec<(String, Value)>)>);
    impl Script {
        fn new(answers: Vec<Result<(u16, Value), String>>) -> Self {
            Self(Mutex::new((answers.into(), Vec::new())))
        }
        fn asked(&self) -> Vec<(String, Value)> {
            self.0.lock().map(|g| g.1.clone()).unwrap_or_default()
        }
    }
    impl Transport for Script {
        fn post(&self, url: &str, body: &Value) -> Result<(u16, Value), String> {
            let Ok(mut g) = self.0.lock() else { return Err("poisoned".into()) };
            g.1.push((url.to_string(), body.clone()));
            g.0.pop_front().unwrap_or(Err("no answer".into()))
        }
    }

    fn started_body() -> Value {
        json!({"deviceCode": "dev-secret", "userCode": "ABCD-EFGH", "verificationUri": "https://console.redrob.ai/connect",
               "verificationUriComplete": "https://console.redrob.ai/connect?code=ABCD-EFGH", "expiresIn": 600, "interval": 5})
    }

    #[test]
    fn authorize_asks_as_cad_and_keeps_the_device_code_inside() {
        let t = Script::new(vec![Ok((200, started_body()))]);
        let r = authorize(&t, "https://c/api");
        assert!(r.is_ok());
        let Ok(s) = r else { return };
        assert_eq!(t.asked(), vec![("https://c/api/device/authorize".to_string(), json!({"product": "cad"}))]);
        assert_eq!((s.user_code.as_str(), s.url.as_str()), ("ABCD-EFGH", "https://console.redrob.ai/connect?code=ABCD-EFGH"));
        assert_eq!((s.expires_in, s.interval), (Duration::from_secs(600), Duration::from_secs(5)));
        assert!(authorize(&Script::new(vec![Ok((400, json!({"error": "invalid_request"})))]), "b").is_err());
        assert!(authorize(&Script::new(vec![Ok((200, json!({"userCode": "X"})))]), "b").is_err());
        assert!(authorize(&Script::new(vec![Err("down".into())]), "b").is_err());
    }

    #[test]
    fn poll_maps_every_console_answer() {
        let s = Started {
            device_code: "dev-secret".into(),
            user_code: "U".into(),
            url: "u".into(),
            expires_in: FALLBACK_EXPIRES,
            interval: FALLBACK_INTERVAL,
        };
        let one = |r: Result<(u16, Value), String>| poll_once(&Script::new(vec![r]), "https://c", &s);
        assert_eq!(one(Ok((400, json!({"error": "authorization_pending"})))), Poll::Pending);
        assert_eq!(one(Ok((400, json!({"error": "slow_down"})))), Poll::SlowDown);
        assert_eq!(one(Ok((403, json!({"error": "access_denied"})))), Poll::Denied);
        assert_eq!(one(Ok((400, json!({"error": "expired_token"})))), Poll::Expired);
        assert_eq!(one(Ok((400, json!({"error": "invalid_grant"})))), Poll::Failed("invalid_grant".into()));
        assert_eq!(one(Ok((500, json!({})))), Poll::Failed("http_500".into()));
        assert_eq!(one(Err("down".into())), Poll::Unreachable);
        assert_eq!(one(Ok((200, json!({"apiKeyName": "k"})))), Poll::Failed("missing_key".into()));
        assert_eq!(
            one(Ok((200, json!({"apiKey": "rk_1", "apiKeyName": "Redrob CAD", "accountName": "Acme"})))),
            Poll::Connected(Stored {
                api_key: "rk_1".into(),
                api_key_name: "Redrob CAD".into(),
                account_name: "Acme".into(),
                api_base_url: "https://c".into()
            })
        );
        let t = Script::new(vec![Ok((400, json!({"error": "authorization_pending"})))]);
        let _ = poll_once(&t, "https://c", &s);
        assert_eq!(t.asked(), vec![("https://c/device/token".to_string(), json!({"deviceCode": "dev-secret"}))]);
    }

    #[test]
    fn the_key_file_round_trips_and_is_private() {
        let dir = std::env::temp_dir().join(format!("redrob-cad-acct-{}", std::process::id()));
        let path = dir.join("account.json");
        let s =
            Stored { api_key: "rk_secret".into(), api_key_name: "Redrob CAD".into(), account_name: "Acme".into(), api_base_url: "https://c".into() };
        assert!(save(&path, &s).is_ok());
        assert_eq!(load(&path), Some(s));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).map(|m| m.permissions().mode() & 0o777).unwrap_or(0);
            assert_eq!(mode, 0o600);
        }
        let mut acct = DeviceAccount::new(Some(path.clone()), None, Arc::new(Script::new(vec![])), "https://c".into(), None);
        assert_eq!(acct.account().map(|a| a.account_name), Some("Acme".into()));
        acct.disconnect();
        assert!(!path.exists() && acct.account().is_none() && acct.generation() == 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_refused_start_is_reported_in_plain_words() {
        let mut acct =
            DeviceAccount::new(None, None, Arc::new(Script::new(vec![Ok((400, json!({"error": "invalid_request"})))])), "https://c".into(), None);
        acct.connect();
        let deadline = Instant::now() + Duration::from_secs(5);
        while matches!(acct.state(), ConnectState::Starting) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(acct.state(), ConnectState::Failed("console.redrob.ai would not start the connection.".into()));
        assert!(acct.account().is_none());
    }

    #[test]
    fn the_shared_launcher_key_is_the_fallback_and_its_changes_restart_the_agent() {
        let dir = std::env::temp_dir().join(format!("redrob-cad-shared-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let own = dir.join("own").join("account.json");
        let shared = dir.join("shared").join("account.json");
        let key = |k: &str, name: &str| Stored {
            api_key: k.into(),
            api_key_name: name.into(),
            account_name: "Acme".into(),
            api_base_url: "https://c".into(),
        };
        let acct = DeviceAccount::new(Some(own.clone()), Some(shared.clone()), Arc::new(Script::new(vec![])), "https://c".into(), None);
        assert!(acct.account().is_none());
        let g0 = acct.generation();
        // Signed in through Redrob Launcher: used, marked as such, and the agent restarts with it.
        assert!(save(&shared, &key("rk_shared", "Redrob Launcher")).is_ok());
        assert_eq!(acct.account().map(|a| (a.key_name, a.via_launcher)), Some(("Redrob Launcher".into(), true)));
        assert_eq!(acct.key_source().current().map(|(s, via)| (s.api_key, via)), Some(("rk_shared".into(), true)));
        assert!(acct.generation() > g0);
        let g1 = acct.generation();
        assert_eq!(acct.generation(), g1, "an unchanged shared key restarts nothing");
        // The app's own key wins.
        assert!(save(&own, &key("rk_own", "Redrob CAD")).is_ok());
        let mut acct = DeviceAccount::new(Some(own.clone()), Some(shared.clone()), Arc::new(Script::new(vec![])), "https://c".into(), None);
        assert_eq!(acct.key_source().current().map(|(s, via)| (s.api_key, via)), Some(("rk_own".into(), false)));
        // Disconnecting here removes only this app's own key; the launcher's file is not this app's to delete.
        acct.disconnect();
        assert!(!own.exists() && shared.exists());
        assert_eq!(acct.key_source().current().map(|(s, _)| s.api_key), Some("rk_shared".into()));
        // Signed out in the launcher: nothing left.
        assert!(std::fs::remove_file(&shared).is_ok());
        assert!(acct.account().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
