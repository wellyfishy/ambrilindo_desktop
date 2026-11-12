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
