//! Test fixtures. Tests never open `tournament_data/db.sqlite3` for writing: they get a
//! consistent snapshot of it (including anything still in the WAL) in a fresh temp folder.

use crate::db::DatabaseManager;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, Connection};
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// The real tournament database, relative to `src-tauri/` (cargo test's working dir).
pub const REAL_DB: &str = "../tournament_data/db.sqlite3";

pub struct TestDb {
    pub db: DatabaseManager,
    pub dir: PathBuf,
    pub path: PathBuf,
}

impl TestDb {
    pub fn pool(&self) -> sqlx::SqlitePool {
        self.db.get_pool_cloned().expect("pool")
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Snapshot `src` into `dest` with `VACUUM INTO` over a read-only connection, so the
/// source is never written and WAL contents are included.
pub async fn snapshot(src: &Path, dest: &Path) -> Result<(), String> {
    let url = format!("sqlite://{}", src.to_string_lossy());
    let mut conn = SqliteConnectOptions::from_str(&url)
        .map_err(|e| e.to_string())?
        .read_only(true)
        .connect()
        .await
        .map_err(|e| format!("open read-only: {e}"))?;
    let target = dest.to_string_lossy().replace('\'', "''");
    sqlx::query(&format!("VACUUM INTO '{target}'"))
        .execute(&mut conn)
        .await
        .map_err(|e| format!("VACUUM INTO: {e}"))?;
    conn.close().await.map_err(|e| e.to_string())?;
    Ok(())
}

/// On Windows the pool can still hold the files when `TestDb` drops, so its folder may
/// survive; remove copies left by earlier runs (older than 10 minutes).
fn sweep_stale_copies() {
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else { return };
    for e in entries.flatten() {
        if !e.file_name().to_string_lossy().starts_with("ambr_test_") {
            continue;
        }
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age.as_secs() > 600);
        if old {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

/// A mounted private copy of the real tournament database, or `None` when it is absent
/// (e.g. on a fresh checkout), so data-dependent tests skip instead of failing.
pub async fn real_db_copy() -> Option<TestDb> {
    let src = Path::new(REAL_DB);
    if !src.exists() {
        eprintln!("skip: {REAL_DB} not found");
        return None;
    }
    sweep_stale_copies();
    let dir = std::env::temp_dir().join(format!("ambr_test_{}", rand::random::<u64>()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("db.sqlite3");
    snapshot(src, &path).await.expect("snapshot real db");
    let mut db = DatabaseManager::new();
    db.mount_database(&path).await.expect("mount copy");
    Some(TestDb { db, dir, path })
}

// ---- in-process arena server ----------------------------------------------------

use crate::server::{create_router, AppState};
use std::sync::Arc;
use tokio::sync::Mutex;

/// The real Axum router on 127.0.0.1:<random port>, backed by a private DB copy. This is
/// exactly what LAN tablets talk to (HTTP `/api/*`, `/api/command/:cmd`, `/ws/...`).
pub struct TestServer {
    pub base: String,
    pub ws_base: String,
    pub state: Arc<AppState>,
    pub http: reqwest::Client,
    pub dir: PathBuf,
}

pub async fn server_on_real_copy() -> Option<TestServer> {
    server_on_real_copy_at("127.0.0.1:0").await
}

pub async fn server_on_real_copy_at(bind: &str) -> Option<TestServer> {
    let t = real_db_copy().await?;
    // keep the folder alive for the server's lifetime; TestDb's Drop would delete it
    let dir = t.dir.clone();
    let path = t.path.clone();
    std::mem::forget(t);
    let mut db = DatabaseManager::new();
    db.mount_database(&path).await.expect("mount copy");
    let (tx, _) = tokio::sync::broadcast::channel(512);
    let state = Arc::new(AppState {
        broadcast_tx: tx,
        db: Arc::new(Mutex::new(db)),
        app_handle: None,
        live_tatami_cache: Arc::new(Mutex::new(Default::default())),
    });
    let listener = tokio::net::TcpListener::bind(bind).await.expect("bind");
    let addr = listener.local_addr().unwrap();
    let app = create_router(state.clone());
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Some(TestServer {
        base: format!("http://{addr}"),
        ws_base: format!("ws://{addr}"),
        state,
        http: reqwest::Client::new(),
        dir,
    })
}

impl TestServer {
    /// POST /api/command/:cmd, the generic dispatcher LAN clients use. Returns (status, body).
    pub async fn cmd(&self, cmd: &str, payload: serde_json::Value) -> (u16, serde_json::Value) {
        let r = self.http.post(format!("{}/api/command/{cmd}", self.base)).json(&payload).send().await.expect("request");
        let code = r.status().as_u16();
        (code, r.json().await.unwrap_or(serde_json::Value::Null))
    }

    /// Like `cmd` but asserts success and returns `data`.
    pub async fn ok(&self, cmd: &str, payload: serde_json::Value) -> serde_json::Value {
        let (code, body) = self.cmd(cmd, payload.clone()).await;
        assert!(code == 200 && body["status"] == "success", "{cmd} {payload} -> {code} {body}");
        body.get("data").cloned().unwrap_or(serde_json::Value::Null)
    }

    pub async fn get(&self, path: &str) -> (u16, serde_json::Value) {
        let r = self.http.get(format!("{}{path}", self.base)).send().await.expect("request");
        let code = r.status().as_u16();
        (code, r.json().await.unwrap_or(serde_json::Value::Null))
    }

    pub async fn post(&self, path: &str, payload: serde_json::Value) -> (u16, serde_json::Value) {
        let r = self.http.post(format!("{}{path}", self.base)).json(&payload).send().await.expect("request");
        let code = r.status().as_u16();
        (code, r.json().await.unwrap_or(serde_json::Value::Null))
    }

    pub async fn pool(&self) -> sqlx::SqlitePool {
        self.state.db.lock().await.get_pool_cloned().expect("pool")
    }
}

// ---- local web portal (Django) for desktop <-> web end-to-end tests --------------

/// Ed25519 seed bytes 0..32, the same test key as the web's `backend/tests.py` (`_TEST_SEED`).
pub const WEB_TEST_SIGNING_KEY: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";
pub const WEB_TEST_LEGACY_TOKEN: &str = "e2e-legacy-token";

/// The web project and its venv python, or None (tests skip) when they are not next to this repo.
fn web_paths() -> Option<(PathBuf, PathBuf)> {
    let web_dir = std::env::var("AMBRILINDO_WEB_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../../ambrilindo_web/ambrilindo_web_pub"));
    let python = if cfg!(windows) { web_dir.join("../venv/Scripts/python.exe") } else { web_dir.join("../venv/bin/python") };
    if !web_dir.join("manage.py").exists() || !python.exists() {
        eprintln!("skip: web project or venv not found at {}", web_dir.display());
        return None;
    }
    Some((web_dir.canonicalize().ok()?, python.canonicalize().ok()?))
}

/// A Django web portal on 127.0.0.1:<free port> with a throw-away SQLite DB (migrated once per
/// test run, copied per server) and the e2e seed (`web_e2e_seed.py`). Never a real web DB:
/// SQLITE_PATH always points into a temp folder, no REDIS_URL (in-memory channel layer).
pub struct WebServer {
    pub base: String,
    pub dir: PathBuf,
    pub seed: serde_json::Value,
    web_dir: PathBuf,
    python: PathBuf,
    db: PathBuf,
    port: u16,
    child: Option<std::process::Child>,
}

impl WebServer {
    pub async fn start() -> Option<WebServer> {
        let (web_dir, python) = web_paths()?;
        let template = migrated_template(&web_dir, &python)?;
        let dir = std::env::temp_dir().join(format!("ambr_test_web_{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).expect("web temp dir");
        let db = dir.join("web.sqlite3");
        std::fs::copy(&template, &db).expect("copy migrated web db");
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let mut s = WebServer { base: format!("http://127.0.0.1:{port}"), dir, seed: serde_json::Value::Null, web_dir, python, db, port, child: None };
        let out = s.shell(include_str!("web_e2e_seed.py"));
        let line = out.lines().find_map(|l| l.strip_prefix("E2E_SEED=")).unwrap_or_else(|| panic!("seed printed nothing:\n{out}"));
        s.seed = serde_json::from_str(line).expect("seed json");
        s.spawn_server();
        s.wait_ready().await;
        Some(s)
    }

    fn command(&self, args: &[&str]) -> std::process::Command {
        let mut c = std::process::Command::new(&self.python);
        c.current_dir(&self.web_dir).args(args);
        web_env(&mut c, &self.db);
        c
    }

    /// Runs `manage.py shell -c <code>` against this server's DB; returns stdout.
    pub fn shell(&self, code: &str) -> String {
        let out = self.command(&["manage.py", "shell", "-c", code]).output().expect("run manage.py shell");
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        assert!(out.status.success(), "manage.py shell failed:\n{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
        stdout
    }

    fn spawn_server(&mut self) {
        let log = std::fs::File::create(self.dir.join("runserver.log")).expect("log file");
        let child = self
            .command(&["manage.py", "runserver", &format!("127.0.0.1:{}", self.port), "--noreload"])
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .spawn()
            .expect("start web runserver");
        self.child = Some(child);
    }

    /// Waits until the web answers HTTP (any status) or panics with its log.
    pub async fn wait_ready(&mut self) {
        let http = reqwest::Client::new();
        for _ in 0..120 {
            if http.get(format!("{}/api/sync/events/", self.base)).send().await.is_ok() {
                return;
            }
            if let Some(Ok(Some(st))) = self.child.as_mut().map(|c| c.try_wait()) {
                panic!("web server exited ({st}):\n{}", self.log());
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        panic!("web server not ready after 60 s:\n{}", self.log());
    }

    pub fn log(&self) -> String {
        std::fs::read_to_string(self.dir.join("runserver.log")).unwrap_or_default()
    }

    /// Stops the web (to test the desktop while the web is unreachable).
    pub fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    /// Starts it again on the same port and DB.
    pub async fn restart(&mut self) {
        self.stop();
        self.spawn_server();
        self.wait_ready().await;
    }

    /// Logs in as the seeded admin and POSTs the "Lisensi Desktop" admin form, the real way an
    /// admin grants/revokes (it also drops the web's cached sync principals).
    pub async fn admin_license_action(&self, fields: &[(&str, String)]) {
        let http = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build().unwrap();
        // Django accepts a CSRF cookie we choose, as long as the header carries the same secret.
        let mut cookies: Vec<(String, String)> = vec![("csrftoken".into(), "e2ecsrfe2ecsrfe2ecsrfe2ecsrfe2ec".into())];
        let cookie_header = |c: &Vec<(String, String)>| c.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("; ");
        let csrf = |c: &Vec<(String, String)>| c.iter().find(|(k, _)| k == "csrftoken").map(|(_, v)| v.clone()).unwrap();
        let absorb = |c: &mut Vec<(String, String)>, resp: &reqwest::Response| {
            for h in resp.headers().get_all(reqwest::header::SET_COOKIE) {
                let raw = h.to_str().unwrap_or_default();
                if let Some((k, v)) = raw.split(';').next().and_then(|kv| kv.split_once('=')) {
                    c.retain(|(n, _)| n != k);
                    c.push((k.to_string(), v.to_string()));
                }
            }
        };
        let admin = &self.seed["admin"];
        let resp = http
            .post(format!("{}/ajax-login/", self.base))
            .header(reqwest::header::COOKIE, cookie_header(&cookies))
            .header("X-CSRFToken", csrf(&cookies))
            .form(&[("username", admin["username"].as_str().unwrap()), ("password", admin["password"].as_str().unwrap())])
            .send()
            .await
            .expect("admin login");
        absorb(&mut cookies, &resp);
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        assert_eq!(body["success"], true, "admin login failed: {body}");
        let resp = http
            .post(format!("{}/admin_dashboard/lisensi/", self.base))
            .header(reqwest::header::COOKIE, cookie_header(&cookies))
            .header("X-CSRFToken", csrf(&cookies))
            .form(fields)
            .send()
            .await
            .expect("admin license form");
        assert!(resp.status().is_redirection(), "admin license form answered {}", resp.status());
    }

    /// Subscribes to an SSE stream of the web and returns once it is connected.
    pub async fn sse(&self, path: &str) -> SseStream {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<u16>();
        let url = format!("{}{}", self.base, path);
        let task = tokio::spawn(async move {
            let mut resp = match reqwest::Client::new().get(&url).send().await {
                Ok(r) => r,
                Err(_) => {
                    let _ = ready_tx.send(0);
                    return;
                }
            };
            let status = resp.status().as_u16();
            let mut ready = Some(ready_tx);
            if status != 200 {
                let _ = ready.take().unwrap().send(status);
                return;
            }
            let mut buf = String::new();
            while let Ok(Some(chunk)) = resp.chunk().await {
                buf.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(end) = buf.find("\n\n") {
                    let frame: String = buf.drain(..end + 2).collect();
                    if frame.starts_with(": connected") {
                        if let Some(r) = ready.take() {
                            let _ = r.send(200);
                        }
                        continue;
                    }
                    if let Some(data) = frame.lines().find_map(|l| l.strip_prefix("data: ")) {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                            let _ = tx.send(v);
                        }
                    }
                }
            }
        });
        let status = tokio::time::timeout(std::time::Duration::from_secs(10), ready_rx).await.ok().and_then(|r| r.ok()).unwrap_or(0);
        SseStream { rx, task, status }
    }
}

impl Drop for WebServer {
    fn drop(&mut self) {
        self.stop();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn web_env(c: &mut std::process::Command, db: &Path) {
    c.env("DEBUG", "True")
        .env("SQLITE_PATH", db)
        .env("LICENSE_SIGNING_KEY", WEB_TEST_SIGNING_KEY)
        .env("API_TOKEN", WEB_TEST_LEGACY_TOKEN)
        .env("SYNC_ALLOW_LEGACY_TOKEN", "true")
        .env("USE_S3", "false")
        .env("FONNTE_SEND_MESSAGES", "false")
        .env("PYTHONUNBUFFERED", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .env_remove("REDIS_URL");
}

/// `manage.py migrate` once per test process into a template DB; each server copies it.
fn migrated_template(web_dir: &Path, python: &Path) -> Option<PathBuf> {
    static TEMPLATE: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);
    let mut t = TEMPLATE.lock().unwrap();
    if let Some(p) = t.as_ref() {
        return Some(p.clone());
    }
    let dir = std::env::temp_dir().join(format!("ambr_test_webtpl_{}", std::process::id()));
    std::fs::create_dir_all(&dir).ok()?;
    let db = dir.join("template.sqlite3");
    let _ = std::fs::remove_file(&db);
    let mut c = std::process::Command::new(python);
    c.current_dir(web_dir).args(["manage.py", "migrate", "--noinput"]);
    web_env(&mut c, &db);
    let out = c.output().expect("run migrate");
    assert!(out.status.success(), "web migrate failed:\n{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    *t = Some(db.clone());
    Some(db)
}

/// Parsed `data: {"command", "details"}` messages of one SSE stream.
pub struct SseStream {
    rx: tokio::sync::mpsc::UnboundedReceiver<serde_json::Value>,
    task: tokio::task::JoinHandle<()>,
    /// HTTP status of the stream (200 = connected; 0 = no connection).
    pub status: u16,
}

impl SseStream {
    /// Next message with this command (others are skipped), or panics after `secs`.
    pub async fn expect(&mut self, command: &str, secs: u64) -> serde_json::Value {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(secs);
        let mut seen = Vec::new();
        loop {
            match tokio::time::timeout_at(deadline, self.rx.recv()).await {
                Ok(Some(v)) if v["command"] == command => return v["details"].clone(),
                Ok(Some(v)) => seen.push(v["command"].to_string()),
                _ => panic!("SSE: no '{command}' within {secs} s (saw {seen:?})"),
            }
        }
    }

    /// Every message that arrives within `ms` (for "nothing was sent" checks).
    pub async fn drain(&mut self, ms: u64) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(ms);
        while let Ok(Some(v)) = tokio::time::timeout_at(deadline, self.rx.recv()).await {
            out.push(v);
        }
        out
    }
}

impl Drop for SseStream {
    fn drop(&mut self) {
        self.task.abort();
    }
}

// ---- smoke test of the harness itself --------------------------------------------

#[tokio::test]
async fn harness_serves_health_and_events() {
    let Some(s) = server_on_real_copy().await else { return };
    let (code, _) = s.get("/api/health").await;
    assert_eq!(code, 200);
    let (code, body) = s.get("/api/events").await;
    assert_eq!(code, 200, "{body}");
    let text = body.to_string();
    assert!(text.contains("PORPROV VIII KALTIM 2026"), "event 17 must be in the copy: {text:.200}");
}
