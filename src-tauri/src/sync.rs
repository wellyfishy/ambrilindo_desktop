use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite};
use std::path::{Path, PathBuf};
use std::time::Instant;
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::Mutex;
use tracing::{info, warn};
use std::sync::atomic::{AtomicI64, Ordering};

/// One HTTP client for every call to the web (connection pool and TLS roots are set up once
/// instead of per request; a client per queue item cost memory and a TLS setup every 12 s).
fn shared_http_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .pool_idle_timeout(std::time::Duration::from_secs(60))
            .pool_max_idle_per_host(2)
            .build()
            .unwrap_or_default()
    })
}

/// The shared client with a per-request timeout (same call shape as `reqwest::Client`).
#[derive(Clone, Copy)]
pub struct TimedClient {
    timeout: std::time::Duration,
}

impl TimedClient {
    pub fn new(timeout: std::time::Duration) -> Self {
        Self { timeout }
    }
    pub fn get(&self, url: &str) -> reqwest::RequestBuilder {
        shared_http_client().get(url).timeout(self.timeout)
    }
    pub fn post(&self, url: &str) -> reqwest::RequestBuilder {
        shared_http_client().post(url).timeout(self.timeout)
    }
}

/// Authorization for one request to the web sync API.
pub struct SyncAuth {
    pub authorization: String,
    pub device_id: Option<String>,
    pub mode: &'static str,
}

impl SyncAuth {
    pub fn apply(&self, rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let rb = rb.header("Authorization", &self.authorization);
        match &self.device_id {
            Some(id) => rb.header("X-Device-Id", id),
            None => rb,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncAuthStatus {
    /// license | legacy | none
    pub mode: String,
    pub message: Option<String>,
    /// Set while the queue is paused because the web refused this computer (401/403).
    pub blocked_reason: Option<String>,
}

/// Tests pick the credentials the desktop sends (the real license is verified against the
/// production public key, and debug builds always have a legacy fallback token).
#[cfg(test)]
pub mod test_auth {
    use super::SyncAuth;
    #[derive(Clone)]
    pub enum Mode {
        License { token: String, device_id: String },
        Legacy(String),
        None,
    }
    static MODE: std::sync::Mutex<Option<Mode>> = std::sync::Mutex::new(None);
    pub fn set(mode: Option<Mode>) {
        *MODE.lock().unwrap() = mode;
        super::SyncService::clear_auth_block();
    }
    pub(super) fn get() -> Option<Result<SyncAuth, String>> {
        let mode = MODE.lock().unwrap().clone()?;
        Some(match mode {
            Mode::License { token, device_id } => Ok(SyncAuth { authorization: format!("License {token}"), device_id: Some(device_id), mode: "license" }),
            Mode::Legacy(t) => Ok(SyncAuth { authorization: format!("Bearer {t}"), device_id: None, mode: "legacy" }),
            Mode::None => Err("Sinkron web butuh lisensi aktif.".to_string()),
        })
    }
}

/// One queue sender per database at a time (see `process_sync_queue`).
struct QueueRun {
    lock: tokio::sync::Mutex<()>,
    rerun: std::sync::atomic::AtomicBool,
}

fn queue_run_for(db_path: Option<&PathBuf>) -> Arc<QueueRun> {
    static RUNS: std::sync::Mutex<Vec<(String, Arc<QueueRun>)>> = std::sync::Mutex::new(Vec::new());
    let key = db_path.map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let mut runs = RUNS.lock().unwrap();
    if let Some((_, r)) = runs.iter().find(|(k, _)| *k == key) {
        return r.clone();
    }
    let r = Arc::new(QueueRun { lock: tokio::sync::Mutex::new(()), rerun: std::sync::atomic::AtomicBool::new(false) });
    runs.push((key, r.clone()));
    r
}

/// After a 401/403 the queue pauses this long instead of hammering the server every tick.
const AUTH_BLOCK_SECS: i64 = 300;
static AUTH_BLOCKED_UNTIL: AtomicI64 = AtomicI64::new(0);
static AUTH_BLOCK_REASON: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SyncConfig {
    pub mode: String, // "local" | "production" | "custom"
    pub url: String,
    pub label: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EventMapping {
    pub hosted_event_id: Option<i64>,
    pub hosted_event_name: Option<String>,
    pub target_mode: String,
    pub is_mismatch: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HostedEventOption {
    pub id: i64,
    pub nama_event: String,
    pub tanggal_mulai: Option<String>,
    pub tanggal_selesai: Option<String>,
    pub tempat: Option<String>,
    pub status: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PingResult {
    pub success: bool,
    pub status_code: Option<u16>,
    pub latency_ms: u64,
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SyncQueueStats {
    pub total_count: i64,
    pub failed_count: i64,
    pub pending_count: i64,
    pub synced_count: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SyncQueueEntry {
    pub id: i64,
    pub event_id: Option<i64>,
    pub endpoint: String,
    pub payload: String,
    pub status: String,
    pub retry_count: i32,
    pub last_error: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    // Parsed quick helpers for table display
    pub kode_realtime: Option<String>,
    pub ring: Option<i32>,
    pub round: Option<i32>,
    pub urutan: Option<i32>,
    pub score: Option<String>,
    pub winner: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PullResult {
    pub success: bool,
    pub message: String,
    pub created_count: usize,
    pub updated_count: usize,
    pub logo_count: usize,
}

pub struct SyncService;

impl SyncService {
    /// Resolves base directory for configuration files (adjacent to db or fallback tournament_data)
    fn get_config_dir(db_path: Option<&PathBuf>) -> PathBuf {
        if let Some(p) = db_path {
            if let Some(parent) = p.parent() {
                return parent.to_path_buf();
            }
        }
        PathBuf::from("tournament_data")
    }

    /// Resolves media directory for contingent logos
    fn get_media_dir(db_path: Option<&PathBuf>) -> PathBuf {
        Self::get_config_dir(db_path).join("media")
    }

    /// Shared legacy token (HOSTED_API_TOKEN) for web servers that still accept it during the move to
    /// license-based sync. Read from the environment or a `.env` next to the app; debug builds fall
    /// back to the local dev server's default. Release builds carry no token of their own.
    fn legacy_api_token() -> Option<String> {
        if let Ok(token) = std::env::var("HOSTED_API_TOKEN").or_else(|_| std::env::var("API_TOKEN")) {
            if !token.trim().is_empty() {
                return Some(token.trim().to_string());
            }
        }
        for path in [PathBuf::from(".env"), PathBuf::from("../.env"), PathBuf::from("tournament_data/.env")] {
            let Ok(content) = std::fs::read_to_string(&path) else { continue };
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('#') {
                    continue;
                }
                if let Some((key, val)) = trimmed.split_once('=') {
                    let k = key.trim();
                    if k == "HOSTED_API_TOKEN" || k == "API_TOKEN" {
                        let v = val.trim().trim_matches('"').trim_matches('\'').trim();
                        if !v.is_empty() {
                            return Some(v.to_string());
                        }
                    }
                }
            }
        }
        #[cfg(debug_assertions)]
        {
            return Some("ambrilindo-secret-api-token-dev".to_string());
        }
        #[allow(unreachable_code)]
        None
    }

    /// How this computer authenticates to the web sync API: the activated license (preferred; the web
    /// only lets it touch the events granted to that license), else the legacy shared token.
    pub fn auth_headers() -> Result<SyncAuth, String> {
        #[cfg(test)]
        if let Some(forced) = test_auth::get() {
            return forced;
        }
        if let Some((token, device_id)) = crate::license::sync_credentials() {
            return Ok(SyncAuth { authorization: format!("License {token}"), device_id: Some(device_id), mode: "license" });
        }
        if let Some(token) = Self::legacy_api_token() {
            return Ok(SyncAuth { authorization: format!("Bearer {token}"), device_id: None, mode: "legacy" });
        }
        Err("Sinkron web butuh lisensi aktif. Aktifkan lisensi di menu Lisensi.".to_string())
    }

    /// Error text for a non-2xx response, using the server's `error` message when it sent one.
    pub async fn http_error(resp: reqwest::Response) -> String {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        let server_msg = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
            .filter(|m| !m.trim().is_empty());
        match server_msg {
            Some(m) => format!("HTTP {}: {}", status.as_u16(), m),
            None => format!("Server web mengembalikan status HTTP {}", status),
        }
    }

    /// Current auth mode for the UI (`get_sync_auth_status`).
    pub fn auth_status() -> SyncAuthStatus {
        let blocked_until = AUTH_BLOCKED_UNTIL.load(Ordering::Relaxed);
        let blocked_reason = if blocked_until > now_secs() { AUTH_BLOCK_REASON.lock().ok().and_then(|g| g.clone()) } else { None };
        match Self::auth_headers() {
            Ok(a) => SyncAuthStatus { mode: a.mode.to_string(), message: None, blocked_reason },
            Err(m) => SyncAuthStatus { mode: "none".into(), message: Some(m), blocked_reason },
        }
    }

    /// Manual send/retry from the UI: try again right away instead of waiting out the back-off.
    pub fn clear_auth_block() {
        AUTH_BLOCKED_UNTIL.store(0, Ordering::Relaxed);
        if let Ok(mut g) = AUTH_BLOCK_REASON.lock() {
            *g = None;
        }
    }

    fn set_auth_block(reason: &str) {
        AUTH_BLOCKED_UNTIL.store(now_secs() + AUTH_BLOCK_SECS, Ordering::Relaxed);
        if let Ok(mut g) = AUTH_BLOCK_REASON.lock() {
            *g = Some(reason.to_string());
        }
    }

    /// Loads sync target server configuration
    pub fn get_sync_config(db_path: Option<&PathBuf>) -> SyncConfig {
        let config_file = Self::get_config_dir(db_path).join("sync_target.json");
        let default_cfg = SyncConfig {
            mode: "local".to_string(),
            url: "http://127.0.0.1:8001".to_string(),
            label: "Dev Lokal (http://127.0.0.1:8001/)".to_string(),
        };

        if !config_file.exists() {
            // Also check legacy path fallback if present
            let legacy_path = Path::new("../ambrilindo/ambrilindo/database/sync_target.json");
            if legacy_path.exists() {
                if let Ok(content) = std::fs::read_to_string(legacy_path) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                        return Self::parse_config_value(val);
                    }
                }
            }
            return default_cfg;
        }

        match std::fs::read_to_string(&config_file) {
            Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(val) => Self::parse_config_value(val),
                Err(_) => default_cfg,
            },
            Err(_) => default_cfg,
        }
    }

    fn parse_config_value(val: serde_json::Value) -> SyncConfig {
        let mode = val.get("mode").and_then(|m| m.as_str()).unwrap_or("local");
        match mode {
            "production" => SyncConfig {
                mode: "production".to_string(),
                url: "https://www.ambrilindo.com".to_string(),
                label: "Live Production (https://www.ambrilindo.com/)".to_string(),
            },
            "custom" => {
                let mut url = val
                    .get("url")
                    .and_then(|u| u.as_str())
                    .unwrap_or("http://127.0.0.1:8001")
                    .trim()
                    .trim_end_matches('/')
                    .to_string();
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    url = format!("http://{}", url);
                }
                SyncConfig {
                    mode: "custom".to_string(),
                    url: url.clone(),
                    label: format!("Kustom ({})", url),
                }
            }
            _ => {
                let url = val
                    .get("url")
                    .and_then(|u| u.as_str())
                    .unwrap_or("http://127.0.0.1:8001")
                    .trim_end_matches('/')
                    .to_string();
                SyncConfig {
                    mode: "local".to_string(),
                    url,
                    label: "Dev Lokal (http://127.0.0.1:8001/)".to_string(),
                }
            }
        }
    }

    /// Sets sync target server configuration and persists to sync_target.json
    pub fn set_sync_config(
        db_path: Option<&PathBuf>,
        mode: &str,
        custom_url: Option<String>,
    ) -> Result<SyncConfig, String> {
        let dir = Self::get_config_dir(db_path);
        let _ = std::fs::create_dir_all(&dir);
        let config_file = dir.join("sync_target.json");

        let cfg = match mode {
            "production" => SyncConfig {
                mode: "production".to_string(),
                url: "https://www.ambrilindo.com".to_string(),
                label: "Live Production (https://www.ambrilindo.com/)".to_string(),
            },
            "custom" => {
                let mut url = custom_url
                    .unwrap_or_else(|| "http://127.0.0.1:8001".to_string())
                    .trim()
                    .trim_end_matches('/')
                    .to_string();
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    url = format!("http://{}", url);
                }
                SyncConfig {
                    mode: "custom".to_string(),
                    url: url.clone(),
                    label: format!("Kustom ({})", url),
                }
            }
            _ => SyncConfig {
                mode: "local".to_string(),
                url: "http://127.0.0.1:8001".to_string(),
                label: "Dev Lokal (http://127.0.0.1:8001/)".to_string(),
            },
        };

        let json_val = serde_json::json!({
            "mode": cfg.mode,
            "url": cfg.url,
        });

        std::fs::write(
            &config_file,
            serde_json::to_string_pretty(&json_val).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("Gagal menyimpan sync_target.json: {}", e))?;

        info!("Target sync berhasil diubah ke: {:?}", cfg);
        Ok(cfg)
    }

    /// Tests connectivity with target server URL
    pub async fn ping_server(url: &str) -> PingResult {
        let client = TimedClient::new(std::time::Duration::from_millis(3500));

        let target_url = format!("{}/api/sync/events/", url.trim_end_matches('/'));
        let auth = match Self::auth_headers() {
            Ok(a) => a,
            Err(message) => return PingResult { success: false, status_code: None, latency_ms: 0, message },
        };
        let start = Instant::now();

        match auth
            .apply(client.get(&target_url))
            .send()
            .await
        {
            Ok(resp) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                let status = resp.status();
                if status.is_success() {
                    PingResult {
                        success: true,
                        status_code: Some(status.as_u16()),
                        latency_ms,
                        message: format!("Terhubung dengan sukses ({} ms)", latency_ms),
                    }
                } else {
                    PingResult {
                        success: false,
                        status_code: Some(status.as_u16()),
                        latency_ms,
                        message: Self::http_error(resp).await,
                    }
                }
            }
            Err(e) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                let msg = if e.is_timeout() {
                    "Koneksi timeout setelah 3.5 detik".to_string()
                } else if e.is_connect() {
                    "Gagal menghubungkan ke server web. Pastikan server aktif.".to_string()
                } else {
                    format!("Error koneksi: {}", e)
                };
                PingResult {
                    success: false,
                    status_code: None,
                    latency_ms,
                    message: msg,
                }
            }
        }
    }

    /// Fetches all public events from hosted server
    pub async fn fetch_hosted_events(url: &str) -> Result<Vec<HostedEventOption>, String> {
        let client = TimedClient::new(std::time::Duration::from_millis(5000));

        let target_url = format!("{}/api/sync/events/", url.trim_end_matches('/'));
        let auth = Self::auth_headers()?;

        let resp = auth
            .apply(client.get(&target_url))
            .send()
            .await
            .map_err(|e| format!("Gagal memuat event dari web server: {}", e))?;

        if !resp.status().is_success() {
            return Err(Self::http_error(resp).await);
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Format JSON tidak valid: {}", e))?;

        if body.get("status").and_then(|s| s.as_str()) != Some("success") {
            let err = body
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("Server web gagal memuat event");
            return Err(err.to_string());
        }

        Ok(Self::parse_hosted_events(&body))
    }

    /// The `events` of `api/sync/events/`. The web sends `tanggal_event` / `tempat_event`; the
    /// older names `tanggal_mulai` / `tempat` are still read. Empty values count as missing.
    fn parse_hosted_events(body: &serde_json::Value) -> Vec<HostedEventOption> {
        let text = |item: &serde_json::Value, keys: &[&str]| {
            keys.iter()
                .filter_map(|k| item.get(*k).and_then(|v| v.as_str()))
                .map(str::trim)
                .find(|v| !v.is_empty())
                .map(str::to_string)
        };
        body.get("events")
            .and_then(|e| e.as_array())
            .map(|arr| arr.iter().filter_map(|item| {
                Some(HostedEventOption {
                    id: item.get("id")?.as_i64()?,
                    nama_event: text(item, &["nama_event"]).unwrap_or_else(|| "Event Tanpa Nama".to_string()),
                    tanggal_mulai: text(item, &["tanggal_event", "tanggal_mulai"]),
                    tanggal_selesai: text(item, &["tanggal_selesai"]),
                    tempat: text(item, &["tempat_event", "tempat"]),
                    status: text(item, &["status"]),
                })
            }).collect())
            .unwrap_or_default()
    }

    /// Reads event mapping configuration from event_mapping.json
    pub fn get_event_mapping(
        db_path: Option<&PathBuf>,
        local_event_id: i64,
        local_event_name: Option<&str>,
        target_mode: &str,
    ) -> EventMapping {
        // Only this app's own mapping file, and only the key of this target mode: the old local
        // app's file and its mode-less `local_<id>` keys may point at another web event, and a
        // full push to the wrong event replaces its brackets.
        let mapping_file = Self::get_config_dir(db_path).join("event_mapping.json");
        let path_to_read = mapping_file.exists().then_some(mapping_file);

        let mut hosted_id = None;
        let mut hosted_name = None;

        if let Some(p) = path_to_read {
            if let Ok(content) = std::fs::read_to_string(&p) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                    let key = format!("local_{}_{}", local_event_id, target_mode);
                    let entry = val.get(&key);
                    if let Some(e) = entry {
                        hosted_id = e.get("hosted_event_id").and_then(|v| v.as_i64());
                        hosted_name = e
                            .get("hosted_event_name")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                    }
                }
            }
        }

        let is_mismatch = match (&hosted_name, local_event_name) {
            (Some(hn), Some(ln)) => {
                let h_clean = hn.trim().to_lowercase();
                let l_clean = ln.trim().to_lowercase();
                !h_clean.is_empty() && !l_clean.is_empty() && h_clean != l_clean
            }
            _ => false,
        };

        EventMapping {
            hosted_event_id: hosted_id,
            hosted_event_name: hosted_name,
            target_mode: target_mode.to_string(),
            is_mismatch,
        }
    }

    /// Drops every web-portal mapping of a local event (all target modes), e.g. after
    /// the event was deleted, so a stale mapping cannot be picked up again.
    pub fn remove_event_mapping(db_path: Option<&PathBuf>, local_event_id: i64) -> Result<(), String> {
        let mapping_file = Self::get_config_dir(db_path).join("event_mapping.json");
        if !mapping_file.exists() {
            return Ok(());
        }
        let content = std::fs::read_to_string(&mapping_file).map_err(|e| e.to_string())?;
        let mut data: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        let exact = format!("local_{}", local_event_id);
        let prefix = format!("local_{}_", local_event_id);
        let mut changed = false;
        if let Some(obj) = data.as_object_mut() {
            let before = obj.len();
            obj.retain(|k, _| k != &exact && !k.starts_with(&prefix));
            changed = obj.len() != before;
        }
        if changed {
            std::fs::write(&mapping_file, serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?)
                .map_err(|e| format!("Gagal menyimpan event_mapping.json: {}", e))?;
        }
        Ok(())
    }

    /// Saves event mapping to event_mapping.json
    pub fn set_event_mapping(
        db_path: Option<&PathBuf>,
        local_event_id: i64,
        hosted_event_id: i64,
        hosted_event_name: &str,
        target_mode: &str,
    ) -> Result<(), String> {
        let dir = Self::get_config_dir(db_path);
        let _ = std::fs::create_dir_all(&dir);
        let mapping_file = dir.join("event_mapping.json");

        let mut data = if mapping_file.exists() {
            let content = std::fs::read_to_string(&mapping_file).unwrap_or_default();
            serde_json::from_str::<serde_json::Value>(&content)
                .unwrap_or_else(|_| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };

        let key = format!("local_{}_{}", local_event_id, target_mode);
        if let Some(obj) = data.as_object_mut() {
            obj.insert(
                key,
                serde_json::json!({
                    "hosted_event_id": hosted_event_id,
                    "hosted_event_name": hosted_event_name.trim(),
                    "target_mode": target_mode,
                }),
            );
        }

        std::fs::write(
            &mapping_file,
            serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("Gagal menyimpan event_mapping.json: {}", e))?;

        info!(
            "Event mapping saved: local #{} -> hosted #{} ({})",
            local_event_id, hosted_event_id, hosted_event_name
        );
        Ok(())
    }

    /// Pulls athletes, categories, clubs, and utusan idempotently from web server
    pub async fn pull_athletes_from_hosted(
        pool: &Pool<Sqlite>,
        db_path: Option<&PathBuf>,
        local_event_id: i64,
    ) -> Result<PullResult, String> {
        let sync_cfg = Self::get_sync_config(db_path);
        let mapping =
            Self::get_event_mapping(db_path, local_event_id, None, &sync_cfg.mode);

        let hosted_event_id = match mapping.hosted_event_id {
            Some(id) => id,
            None => {
                return Err(
                    "Event lokal belum dihubungkan ke Event di Web Public. Silakan hubungkan event terlebih dahulu.".to_string(),
                );
            }
        };

        let client = TimedClient::new(std::time::Duration::from_secs(30));

        let target_url = format!(
            "{}/api/sync/export-atlet/{}/",
            sync_cfg.url.trim_end_matches('/'),
            hosted_event_id
        );
        let auth = Self::auth_headers()?;

        let resp = auth
            .apply(client.get(&target_url))
            .send()
            .await
            .map_err(|e| format!("Gagal koneksi ke server web saat menarik atlet: {}", e))?;

        if !resp.status().is_success() {
            return Err(Self::http_error(resp).await);
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Format respon tidak valid: {}", e))?;

        if body.get("status").and_then(|s| s.as_str()) != Some("success") {
            let err = body
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("Server web mengembalikan status gagal");
            return Err(err.to_string());
        }

        let mut created_count = 0;
        let mut updated_count = 0;
        let mut utusan_logo_tasks = Vec::new();

        // Steps 1-4 are one transaction: a failure part-way leaves nothing half pulled.
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        // 1. Sync Perguruan
        if let Some(perguruans) = body.get("perguruans").and_then(|p| p.as_array()) {
            for p_val in perguruans {
                if let Some(p_name) = p_val.as_str() {
                    let clean = p_name.trim().to_uppercase();
                    if !clean.is_empty() {
                        let existing: Option<i64> = sqlx::query_scalar(
                            "SELECT id FROM backend_perguruan WHERE event_id = ? AND UPPER(nama_perguruan) = ?"
                        )
                        .bind(local_event_id)
                        .bind(&clean)
                        .fetch_optional(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;

                        if existing.is_none() {
                            sqlx::query(
                                "INSERT INTO backend_perguruan (event_id, nama_perguruan) VALUES (?, ?)"
                            )
                            .bind(local_event_id)
                            .bind(&clean)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
        }

        // 2. Sync Utusan (with logo tasks)
        if let Some(utusans_detail) = body.get("utusans_detail").and_then(|u| u.as_array()) {
            for u_item in utusans_detail {
                let name = u_item
                    .get("nama_utusan")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_uppercase();
                let logo_url = u_item
                    .get("logo_url")
                    .and_then(|l| l.as_str())
                    .map(|s| s.trim().to_string());

                if !name.is_empty() {
                    let existing_id: Option<i64> = sqlx::query_scalar(
                        "SELECT id FROM backend_utusan WHERE event_id = ? AND UPPER(nama_utusan) = ?"
                    )
                    .bind(local_event_id)
                    .bind(&name)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;

                    let u_id = match existing_id {
                        Some(id) => id,
                        None => {
                            let res = sqlx::query(
                                "INSERT INTO backend_utusan (event_id, nama_utusan, logo) VALUES (?, ?, NULL)"
                            )
                            .bind(local_event_id)
                            .bind(&name)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| e.to_string())?;
                            res.last_insert_rowid()
                        }
                    };

                    if let Some(l_url) = logo_url {
                        if !l_url.is_empty() {
                            utusan_logo_tasks.push((u_id, name.clone(), l_url));
                        }
                    }
                }
            }
        } else if let Some(utusans) = body.get("utusans").and_then(|u| u.as_array()) {
            for u_val in utusans {
                if let Some(u_name) = u_val.as_str() {
                    let clean = u_name.trim().to_uppercase();
                    if !clean.is_empty() {
                        let existing: Option<i64> = sqlx::query_scalar(
                            "SELECT id FROM backend_utusan WHERE event_id = ? AND UPPER(nama_utusan) = ?"
                        )
                        .bind(local_event_id)
                        .bind(&clean)
                        .fetch_optional(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;

                        if existing.is_none() {
                            sqlx::query(
                                "INSERT INTO backend_utusan (event_id, nama_utusan, logo) VALUES (?, ?, NULL)"
                            )
                            .bind(local_event_id)
                            .bind(&clean)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
        }

        // 3. Sync Nomor Tanding
        if let Some(nomor_tandings) = body.get("nomor_tandings").and_then(|nt| nt.as_array()) {
            for nt_item in nomor_tandings {
                let name = nt_item
                    .get("nama_nomor_tanding")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .trim();
                let is_bob = nt_item
                    .get("is_bob")
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false);

                if !name.is_empty() {
                    let existing: Option<(i64, bool)> = sqlx::query_as(
                        "SELECT id, is_bob FROM backend_nomortanding WHERE event_id = ? AND nama_nomor_tanding = ?"
                    )
                    .bind(local_event_id)
                    .bind(name)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;

                    match existing {
                        Some((id, cur_bob)) => {
                            if cur_bob != is_bob {
                                sqlx::query(
                                    "UPDATE backend_nomortanding SET is_bob = ? WHERE id = ?"
                                )
                                .bind(is_bob)
                                .bind(id)
                                .execute(&mut *tx)
                                .await
                                .map_err(|e| e.to_string())?;
                            }
                        }
                        None => {
                            sqlx::query(
                                "INSERT INTO backend_nomortanding (event_id, nama_nomor_tanding, is_bob, has_vr) VALUES (?, ?, ?, 0)"
                            )
                            .bind(local_event_id)
                            .bind(name)
                            .bind(is_bob)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
        }

        // 4. Sync Atlet (idempotent update or insert).
        // The web's `public_detail_id` is the key: a number for an individual entry, "B_<id>" for
        // a team entry; team members get "MEMBER_<team>_<id>" so drawing and rosters skip them
        // (same codes as the old local app). An empty `kode_atlet` / `nik` is "no code", never a
        // match: matching '' merged every athlete without a code into one row. A name only matches
        // a row of the same contingent that is not linked to a web entry yet, and no local row is
        // claimed twice in one pull.
        let mut claimed: std::collections::HashSet<i64> = std::collections::HashSet::new();
        if let Some(atlets_data) = body.get("atlets").and_then(|a| a.as_array()) {
            for a_item in atlets_data {
                let text = |key: &str| {
                    a_item.get(key).and_then(|v| v.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
                };
                let kode_atlet = text("kode_atlet");
                let nik = text("nik");
                let perguruan_name = text("perguruan").map(|s| s.to_uppercase()).unwrap_or_default();
                let utusan_name = text("utusan").map(|s| s.to_uppercase()).unwrap_or_default();
                let nt_name = text("nomor_tanding").unwrap_or_default();
                let public_detail_id = match a_item.get("public_detail_id") {
                    Some(serde_json::Value::Number(n)) => Some(n.to_string()),
                    Some(serde_json::Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
                    _ => None,
                };
                let is_member = a_item.get("is_beregu_member").and_then(|b| b.as_bool()).unwrap_or(false);
                let beregu_id = a_item.get("beregu_id").and_then(|b| b.as_i64());
                let is_priority = a_item.get("is_priority").and_then(|ip| ip.as_bool()).unwrap_or(false);

                let mut nama = text("nama_atlet").unwrap_or_default();
                if nama.is_empty() {
                    nama = match &kode_atlet {
                        Some(k) => format!("ATLET {}", k),
                        None => "-".to_string(),
                    };
                }
                if nt_name.is_empty() {
                    continue;
                }

                // Member code = "MEMBER_" + the team entry's code ("B_<team>") + "_" + its id,
                // the format the old local app stored (e.g. MEMBER_B_4_1).
                let additional_code_val: Option<String> = if is_member {
                    public_detail_id.as_ref().map(|p| match beregu_id {
                        Some(b) => format!("MEMBER_B_{b}_{p}"),
                        None => format!("MEMBER_{p}"),
                    })
                } else {
                    public_detail_id.clone()
                };

                // Resolve Perguruan ID
                let perguruan_id: Option<i64> = if !perguruan_name.is_empty() {
                    sqlx::query_scalar(
                        "SELECT id FROM backend_perguruan WHERE event_id = ? AND UPPER(nama_perguruan) = ?"
                    )
                    .bind(local_event_id)
                    .bind(&perguruan_name)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?
                } else {
                    None
                };

                // Resolve Utusan ID
                let utusan_id: Option<i64> = if !utusan_name.is_empty() {
                    sqlx::query_scalar(
                        "SELECT id FROM backend_utusan WHERE event_id = ? AND UPPER(nama_utusan) = ?"
                    )
                    .bind(local_event_id)
                    .bind(&utusan_name)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?
                } else {
                    None
                };

                // Resolve Nomor Tanding ID
                let nt_id: Option<i64> = sqlx::query_scalar(
                    "SELECT id FROM backend_nomortanding WHERE event_id = ? AND nama_nomor_tanding = ?"
                )
                .bind(local_event_id)
                .bind(&nt_name)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;

                let Some(nomor_tanding_id) = nt_id else {
                    continue;
                };

                // Candidate rows of this category, first one not yet claimed in this pull.
                let mut existing_atlet_id: Option<i64> = None;
                let mut lookups: Vec<(&str, String)> = Vec::new();
                if let Some(code) = &additional_code_val {
                    lookups.push(("additional_code = ?", code.clone()));
                }
                if is_member {
                    // a member stored with its bare id by an earlier desktop pull: repair it
                    if let Some(p) = &public_detail_id {
                        lookups.push(("additional_code = ?", p.clone()));
                    }
                }
                if let Some(k) = &kode_atlet {
                    lookups.push(("kode_atlet = ?", k.clone()));
                }
                for (cond, val) in lookups {
                    let ids: Vec<i64> = sqlx::query_scalar(&format!(
                        "SELECT id FROM backend_atlet WHERE event_id = ? AND nomor_tanding_id = ? AND {cond} ORDER BY id"
                    ))
                    .bind(local_event_id)
                    .bind(nomor_tanding_id)
                    .bind(&val)
                    .fetch_all(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
                    if let Some(id) = ids.into_iter().find(|id| !claimed.contains(id)) {
                        existing_atlet_id = Some(id);
                        break;
                    }
                }
                if existing_atlet_id.is_none() && nama != "-" && !nama.starts_with("ATLET ") {
                    // An athlete typed in locally: same name and contingent, not linked to the web.
                    // A team member may also be a row of the same team stored by the old local
                    // app with another member suffix.
                    let member_prefix = match (is_member, beregu_id) {
                        (true, Some(b)) => format!("MEMBER\\_B\\_{b}\\_%"),
                        _ => String::new(),
                    };
                    let ids: Vec<i64> = sqlx::query_scalar(
                        "SELECT id FROM backend_atlet
                          WHERE event_id = ? AND nomor_tanding_id = ? AND LOWER(nama_atlet) = LOWER(?)
                            AND utusan_id IS ?
                            AND (COALESCE(additional_code, '') = '' OR (? != '' AND additional_code LIKE ? ESCAPE '\\'))
                          ORDER BY id"
                    )
                    .bind(local_event_id)
                    .bind(nomor_tanding_id)
                    .bind(&nama)
                    .bind(utusan_id)
                    .bind(&member_prefix)
                    .bind(&member_prefix)
                    .fetch_all(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
                    existing_atlet_id = ids.into_iter().find(|id| !claimed.contains(id));
                }

                match existing_atlet_id {
                    Some(a_id) => {
                        sqlx::query(
                            "UPDATE backend_atlet SET nama_atlet = ?, perguruan_id = ?, utusan_id = ?, nik = ?, kode_atlet = ?, additional_code = ?, is_priority = ? WHERE id = ?"
                        )
                        .bind(&nama)
                        .bind(perguruan_id)
                        .bind(utusan_id)
                        .bind(nik.as_deref())
                        .bind(kode_atlet.as_deref())
                        .bind(additional_code_val.as_deref())
                        .bind(is_priority)
                        .bind(a_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;
                        claimed.insert(a_id);
                        updated_count += 1;
                    }
                    None => {
                        let res = sqlx::query(
                            "INSERT INTO backend_atlet (event_id, nama_atlet, nik, kode_atlet, additional_code, is_priority, perguruan_id, utusan_id, nomor_tanding_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
                        )
                        .bind(local_event_id)
                        .bind(&nama)
                        .bind(nik.as_deref())
                        .bind(kode_atlet.as_deref())
                        .bind(additional_code_val.as_deref())
                        .bind(is_priority)
                        .bind(perguruan_id)
                        .bind(utusan_id)
                        .bind(nomor_tanding_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;
                        claimed.insert(res.last_insert_rowid());
                        created_count += 1;
                    }
                }
            }
        }

        tx.commit().await.map_err(|e| format!("Gagal menyimpan data tarikan: {}", e))?;
        // 5. Download Utusan Logos
        let mut logo_count = 0;
        let media_dir = Self::get_media_dir(db_path).join("logo_utusan");
        let _ = std::fs::create_dir_all(&media_dir);

        let img_client = TimedClient::new(std::time::Duration::from_secs(10));

        for (u_id, u_name, logo_url) in utusan_logo_tasks {
            let mut target_url = logo_url;
            if target_url.starts_with('/') {
                target_url = format!("{}{}", sync_cfg.url.trim_end_matches('/'), target_url);
            }

            let clean_slug = u_name
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '_' })
                .collect::<String>()
                .to_lowercase();
            let ext = if target_url.ends_with(".png") {
                "png"
            } else if target_url.ends_with(".jpg") || target_url.ends_with(".jpeg") {
                "jpg"
            } else {
                "webp"
            };

            let file_name = format!("{}_{}.{}", clean_slug, u_id, ext);
            let local_dest = media_dir.join(&file_name);

            if !local_dest.exists() {
                if let Ok(resp) = img_client.get(&target_url).send().await {
                    if resp.status().is_success() {
                        if let Ok(bytes) = resp.bytes().await {
                            if std::fs::write(&local_dest, &bytes).is_ok() {
                                let rel_path = format!("logo_utusan/{}", file_name);
                                let _ = sqlx::query(
                                    "UPDATE backend_utusan SET logo = ? WHERE id = ?"
                                )
                                .bind(&rel_path)
                                .bind(u_id)
                                .execute(pool)
                                .await;
                                logo_count += 1;
                            }
                        }
                    }
                }
            }
        }

        let hosted_label = mapping
            .hosted_event_name
            .map(|n| format!("'{}' (ID {})", n, hosted_event_id))
            .unwrap_or_else(|| format!("ID #{}", hosted_event_id));

        let mut msg = format!(
            "Berhasil menarik data dari {}: {} atlet baru ditambahkan, {} diperbarui (Total: {} atlet)",
            hosted_label,
            created_count,
            updated_count,
            created_count + updated_count
        );
        if logo_count > 0 {
            msg.push_str(&format!(", {} logo kontingen diunduh", logo_count));
        }
        msg.push('.');

        info!("{}", msg);

        Ok(PullResult {
            success: true,
            message: msg,
            created_count,
            updated_count,
            logo_count,
        })
    }

    /// Pushes brackets & matches to web server
    /// `DatabaseManager::podium_fields` on a pooled connection.
    async fn podium_of(
        pool: &Pool<Sqlite>,
        bagan_id: i64,
    ) -> Result<Option<serde_json::Map<String, serde_json::Value>>, String> {
        let mut conn = pool.acquire().await.map_err(|e| format!("Gagal membuka database: {}", e))?;
        Ok(crate::db::DatabaseManager::podium_fields(&mut conn, bagan_id).await)
    }

    /// Queue endpoints whose payload gets the web `event_id` when it is sent.
    fn needs_web_event_id(endpoint: &str) -> bool {
        ["tatami-status", "api/status/", "api/result/", "api/reopen/", "api/final-result/"]
            .iter()
            .any(|e| endpoint.contains(e))
    }

    pub async fn push_bagan_to_hosted(
        pool: &Pool<Sqlite>,
        db_path: Option<&PathBuf>,
        local_event_id: i64,
        bagan_ids: Option<Vec<i64>>,
    ) -> Result<String, String> {
        let sync_cfg = Self::get_sync_config(db_path);
        let mapping =
            Self::get_event_mapping(db_path, local_event_id, None, &sync_cfg.mode);

        let hosted_event_id = match mapping.hosted_event_id {
            Some(id) => id,
            None => {
                return Err(
                    "Event lokal belum dihubungkan ke Event di Web Public. Silakan pilih event di tombol Hubungkan ke Event Web terlebih dahulu.".to_string(),
                );
            }
        };

        let event_code: String = sqlx::query_scalar(
            "SELECT COALESCE(kode_event, 'EV' || id) FROM backend_event WHERE id = ?"
        )
        .bind(local_event_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Gagal memuat kode event: {}", e))?;

        // Query bagans
        let bagan_query = if let Some(ref ids) = bagan_ids {
            if ids.is_empty() {
                return Err("Tidak ada bagan yang dipilih untuk dikirim.".to_string());
            }
            let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            format!(
                "SELECT b.id, b.nama_bagan, b.tipe_tanding, b.pool, b.kode, b.is_bob, \
                 nt.nama_nomor_tanding \
                 FROM backend_bagan b \
                 LEFT JOIN backend_nomortanding nt ON b.nomor_tanding_id = nt.id \
                 WHERE b.event_id = ? AND b.id IN ({}) ORDER BY b.id ASC",
                placeholders
            )
        } else {
            "SELECT b.id, b.nama_bagan, b.tipe_tanding, b.pool, b.kode, b.is_bob, \
             nt.nama_nomor_tanding \
             FROM backend_bagan b \
             LEFT JOIN backend_nomortanding nt ON b.nomor_tanding_id = nt.id \
             WHERE b.event_id = ? ORDER BY b.id ASC".to_string()
        };

        let mut b_query = sqlx::query(&bagan_query).bind(local_event_id);
        if let Some(ref ids) = bagan_ids {
            for id in ids {
                b_query = b_query.bind(id);
            }
        }

        let bagan_rows = b_query
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Gagal memuat data bagan: {}", e))?;

        if bagan_rows.is_empty() {
            return Err("Tidak ada bagan yang ditemukan untuk dikirim.".to_string());
        }

        let mut bagans_payload = Vec::new();

        for b_row in bagan_rows {
            let b_id: i64 = b_row.get("id");
            let b_nama: Option<String> = b_row.get("nama_bagan");
            let b_tipe: Option<String> = b_row.get("tipe_tanding");
            let b_pool: i32 = b_row.get("pool");
            let b_kode: Option<String> = b_row.get("kode");
            let b_is_bob: bool = b_row.get("is_bob");
            let nt_name: Option<String> = b_row.get("nama_nomor_tanding");

            // Fetch details for this bagan
            let detail_rows = sqlx::query(
                "SELECT d.id, d.round, d.urutan, d.score1, d.score2, d.scorekecil1, d.scorekecil2, \
                 d.vr1, d.vr2, d.kata1, d.kata2, d.pemenang, d.selesai, \
                 a1.nama_atlet as a1_nama, a1.kode_atlet as a1_kode, a1.additional_code as a1_add, a1.is_priority as a1_prio, \
                 p1.nama_perguruan as a1_perg, u1.nama_utusan as a1_uts, \
                 a2.nama_atlet as a2_nama, a2.kode_atlet as a2_kode, a2.additional_code as a2_add, a2.is_priority as a2_prio, \
                 p2.nama_perguruan as a2_perg, u2.nama_utusan as a2_uts \
                 FROM backend_detailbagan d \
                 LEFT JOIN backend_atlet a1 ON d.atlet1_id = a1.id \
                 LEFT JOIN backend_perguruan p1 ON a1.perguruan_id = p1.id \
                 LEFT JOIN backend_utusan u1 ON a1.utusan_id = u1.id \
                 LEFT JOIN backend_atlet a2 ON d.atlet2_id = a2.id \
                 LEFT JOIN backend_perguruan p2 ON a2.perguruan_id = p2.id \
                 LEFT JOIN backend_utusan u2 ON a2.utusan_id = u2.id \
                 WHERE d.bagan_id = ? AND COALESCE(d.team, 0) = 0 ORDER BY d.round ASC, d.urutan ASC"
            )
            .bind(b_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Gagal memuat detail bagan #{}: {}", b_id, e))?;

            let mut dbs_payload = Vec::new();
            for d_row in detail_rows {
                let d_id: i64 = d_row.get("id");
                let round: Option<i32> = d_row.get("round");
                let urutan: Option<i32> = d_row.get("urutan");
                let score1: Option<String> = d_row.get("score1");
                let score2: Option<String> = d_row.get("score2");
                let scorekecil1: Option<String> = d_row.get("scorekecil1");
                let scorekecil2: Option<String> = d_row.get("scorekecil2");
                let vr1: bool = d_row.get("vr1");
                let vr2: bool = d_row.get("vr2");
                let kata1: Option<String> = d_row.get("kata1");
                let kata2: Option<String> = d_row.get("kata2");
                let pemenang: Option<String> = d_row.get("pemenang");
                let selesai: bool = d_row.get("selesai");

                let a1_nama: Option<String> = d_row.get("a1_nama");
                let a1_kode: Option<String> = d_row.get("a1_kode");
                let a1_add: Option<String> = d_row.get("a1_add");
                let a1_prio: Option<bool> = d_row.get("a1_prio");
                let a1_perg: Option<String> = d_row.get("a1_perg");
                let a1_uts: Option<String> = d_row.get("a1_uts");

                let a2_nama: Option<String> = d_row.get("a2_nama");
                let a2_kode: Option<String> = d_row.get("a2_kode");
                let a2_add: Option<String> = d_row.get("a2_add");
                let a2_prio: Option<bool> = d_row.get("a2_prio");
                let a2_perg: Option<String> = d_row.get("a2_perg");
                let a2_uts: Option<String> = d_row.get("a2_uts");

                let kode_realtime = format!("{}-{}-{}", event_code, b_id, d_id);

                dbs_payload.push(serde_json::json!({
                    "round": round,
                    "urutan": urutan,
                    "atlet1_nama": a1_nama.unwrap_or_default(),
                    "atlet1_kode": a1_kode.unwrap_or_default(),
                    "atlet1_perguruan": a1_perg.unwrap_or_default(),
                    "atlet1_utusan": a1_uts.unwrap_or_default(),
                    "atlet1_public_detail_id": a1_add.unwrap_or_default(),
                    "atlet1_is_priority": a1_prio.unwrap_or(false),
                    "atlet2_nama": a2_nama.unwrap_or_default(),
                    "atlet2_kode": a2_kode.unwrap_or_default(),
                    "atlet2_perguruan": a2_perg.unwrap_or_default(),
                    "atlet2_utusan": a2_uts.unwrap_or_default(),
                    "atlet2_public_detail_id": a2_add.unwrap_or_default(),
                    "atlet2_is_priority": a2_prio.unwrap_or(false),
                    "kode_realtime": kode_realtime,
                    "score1": score1,
                    "score2": score2,
                    "scorekecil1": scorekecil1,
                    "scorekecil2": scorekecil2,
                    "vr1": vr1,
                    "vr2": vr2,
                    "kata1": kata1,
                    "kata2": kata2,
                    "selesai": selesai,
                    "pemenang": pemenang,
                }));
            }

            let mut bagan_payload = serde_json::json!({
                "kode": b_kode.unwrap_or_default(),
                "nama_bagan": b_nama,
                "nomor_tanding": nt_name.unwrap_or_default(),
                "is_bob": b_is_bob,
                "tipe_tanding": b_tipe,
                "pool": b_pool,
                "detail_bagans": dbs_payload,
            });
            if let Some(podium) = Self::podium_of(pool, b_id).await? {
                bagan_payload.as_object_mut().expect("object").extend(podium);
            }
            bagans_payload.push(bagan_payload);
        }

        // Every kode this event still has: on a partial push the web removes the pushed categories'
        // brackets that are no longer here (re-drawn), and nothing else.
        let known_kodes: Vec<String> = sqlx::query_scalar(
            "SELECT kode FROM backend_bagan WHERE event_id = ? AND kode IS NOT NULL AND kode <> ''"
        )
        .bind(local_event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat kode bagan: {}", e))?;

        let is_full_sync = bagan_ids.is_none();
        let payload = serde_json::json!({
            "event_pk": hosted_event_id,
            "bagans": bagans_payload,
            "is_full_sync": is_full_sync,
            "known_kodes": known_kodes,
        });

        let client = TimedClient::new(std::time::Duration::from_secs(30));

        let target_url = format!("{}/api/sync/push-bagan/", sync_cfg.url.trim_end_matches('/'));
        let auth = Self::auth_headers()?;

        let resp = auth
            .apply(client.post(&target_url))
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Gagal mengirim bagan ke server web: {}", e))?;

        if !resp.status().is_success() {
            return Err(Self::http_error(resp).await);
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Respon server tidak valid: {}", e))?;

        if body.get("status").and_then(|s| s.as_str()) != Some("success") {
            let err = body
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("Server web gagal menyimpan bagan");
            return Err(err.to_string());
        }

        let msg = body
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Bagan berhasil dikirim ke server web public.")
            .to_string();

        info!("{}", msg);
        Ok(msg)
    }

    /// Sends the Jadwal (days, rows, cells) and every tatami's live status to the web's
    /// "Roster & live tatami" (`api/sync/push-roster/`, same payload as the old local app).
    /// The web replaces its roster for the event and links cells to bagans by name.
    pub async fn push_roster_to_hosted(
        pool: &Pool<Sqlite>,
        db_path: Option<&PathBuf>,
        local_event_id: i64,
    ) -> Result<String, String> {
        let sync_cfg = Self::get_sync_config(db_path);
        let mapping = Self::get_event_mapping(db_path, local_event_id, None, &sync_cfg.mode);
        let Some(hosted_event_id) = mapping.hosted_event_id else {
            return Err("Event lokal belum dihubungkan ke Event di Web Public. Silakan pilih event di tombol Hubungkan ke Event Web terlebih dahulu.".to_string());
        };

        let mut conn = pool.acquire().await.map_err(|e| e.to_string())?;

        // 1. Tatamis with their live status
        let tatami_numbers: Vec<i64> = sqlx::query_scalar(
            "SELECT tatami_number FROM backend_tatami WHERE event_id = ? AND tatami_number IS NOT NULL ORDER BY tatami_number",
        )
        .bind(local_event_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| format!("Gagal memuat tatami: {}", e))?;
        let mut tatamis = Vec::new();
        for n in &tatami_numbers {
            let live = crate::db::DatabaseManager::tatami_live_payload(&mut conn, local_event_id, *n).await;
            tatamis.push(serde_json::json!({
                "tatami_number": n,
                "is_active": live["is_active"],
                "current_category": live["category_name"],
                "current_match_label": live["match_label"],
                "current_atlet_aka": live["atlet_aka"],
                "current_atlet_ao": live["atlet_ao"],
                "kode_realtime": live["kode_realtime"],
            }));
        }

        // Entries per category (teams count once: their members carry a MEMBER_ code)
        let counts: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT nomor_tanding_id, COUNT(*) FROM backend_atlet
              WHERE event_id = ? AND nomor_tanding_id IS NOT NULL
                AND (additional_code IS NULL OR additional_code NOT LIKE 'MEMBER_%')
              GROUP BY nomor_tanding_id",
        )
        .bind(local_event_id)
        .fetch_all(&mut *conn)
        .await
        .unwrap_or_default();
        let count_of: std::collections::HashMap<i64, i64> = counts.into_iter().collect();

        // 2. Days, rows, cells
        let days: Vec<(i64, i64, Option<String>)> = sqlx::query_as(
            "SELECT id, [order], tanggal FROM backend_timetableday WHERE event_id = ? ORDER BY [order], id",
        )
        .bind(local_event_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| format!("Gagal memuat jadwal: {}", e))?;
        let mut days_payload = Vec::new();
        let mut cell_total = 0usize;
        for (day_id, order, tanggal) in days {
            let rows: Vec<(i64, i64, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
                "SELECT id, [order], row_type, time_label, label_text FROM backend_timetablerow WHERE day_id = ? ORDER BY [order], id",
            )
            .bind(day_id)
            .fetch_all(&mut *conn)
            .await
            .unwrap_or_default();
            let mut rows_payload = Vec::new();
            for (row_id, row_order, row_type, time_label, label_text) in rows {
                let row_type = row_type.unwrap_or_else(|| "slot".into());
                let mut cells_payload = Vec::new();
                if row_type == "slot" {
                    let cells: Vec<(Option<i64>, Option<i64>, Option<String>, Option<String>)> = sqlx::query_as(
                        "SELECT t.tatami_number, c.nomor_tanding_id, nt.nama_nomor_tanding, c.custom_text
                           FROM backend_timetablecell c
                           JOIN backend_tatami t ON c.tatami_id = t.id
                           LEFT JOIN backend_nomortanding nt ON c.nomor_tanding_id = nt.id
                          WHERE c.row_id = ?
                          ORDER BY t.tatami_number",
                    )
                    .bind(row_id)
                    .fetch_all(&mut *conn)
                    .await
                    .unwrap_or_default();
                    for (t_num, nt_id, nt_name, custom_text) in cells {
                        let Some(t_num) = t_num else { continue };
                        let name = nt_name.clone().or_else(|| custom_text.clone()).unwrap_or_default();
                        let atlet = nt_id.and_then(|id| count_of.get(&id).copied()).unwrap_or(0);
                        let bouts = if atlet > 1 { atlet - 1 } else { atlet };
                        cells_payload.push(serde_json::json!({
                            "tatami_number": t_num,
                            "nomor_tanding_name": name,
                            "category_name": name,
                            "total_matches": bouts,
                            "total_atlets": atlet,
                            "estimated_time": custom_text.clone().unwrap_or_default(),
                            "custom_text": custom_text.unwrap_or_default(),
                        }));
                    }
                    cell_total += cells_payload.len();
                }
                rows_payload.push(serde_json::json!({
                    "order": row_order,
                    "row_type": row_type,
                    "time_label": time_label.unwrap_or_default(),
                    "label_text": label_text.unwrap_or_default(),
                    "cells": cells_payload,
                }));
            }
            let label = match &tanggal {
                Some(t) if !t.trim().is_empty() => format!("Hari {} ({})", order + 1, t),
                _ => format!("Hari {}", order + 1),
            };
            days_payload.push(serde_json::json!({
                "order": order,
                "tanggal": tanggal.filter(|t| !t.trim().is_empty()),
                "label": label,
                "rows": rows_payload,
            }));
        }
        drop(conn);

        if days_payload.is_empty() {
            return Err("Jadwal event ini masih kosong. Buat jadwal di channel Jadwal dulu.".to_string());
        }

        let payload = serde_json::json!({
            "event_id": hosted_event_id,
            "tatamis": tatamis,
            "days": days_payload,
        });
        let client = TimedClient::new(std::time::Duration::from_secs(30));
        let target_url = format!("{}/api/sync/push-roster/", sync_cfg.url.trim_end_matches('/'));
        let auth = Self::auth_headers()?;
        let resp = auth
            .apply(client.post(&target_url))
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Gagal mengirim roster ke server web: {}", e))?;
        if !resp.status().is_success() {
            return Err(Self::http_error(resp).await);
        }
        let body: serde_json::Value = resp.json().await.map_err(|e| format!("Respon server tidak valid: {}", e))?;
        if body.get("status").and_then(|s| s.as_str()) != Some("success") {
            return Err(body.get("error").and_then(|e| e.as_str()).unwrap_or("Server web gagal menyimpan roster").to_string());
        }
        let msg = body
            .get("message")
            .and_then(|m| m.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("Roster terkirim ({} sel jadwal).", cell_total));
        info!("{}", msg);
        Ok(msg)
    }

    /// Force synchronizes finished matches and champions snapshot to web server
    pub async fn force_sync_results_to_hosted(
        pool: &Pool<Sqlite>,
        db_path: Option<&PathBuf>,
        local_event_id: i64,
    ) -> Result<String, String> {
        let sync_cfg = Self::get_sync_config(db_path);
        let mapping =
            Self::get_event_mapping(db_path, local_event_id, None, &sync_cfg.mode);

        let hosted_event_id = match mapping.hosted_event_id {
            Some(id) => id,
            None => {
                return Err(
                    "Event lokal belum dihubungkan ke Event di Web Public. Silakan pilih event di Hubungkan ke Event Web terlebih dahulu.".to_string(),
                );
            }
        };

        let event_code: String = sqlx::query_scalar(
            "SELECT COALESCE(kode_event, 'EV' || id) FROM backend_event WHERE id = ?"
        )
        .bind(local_event_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Gagal memuat kode event: {}", e))?;

        // 1. Gather all finished DetailBagan (bouts inside a team match are not on the web), in
        //    the order they were played: pools, then the Final bagan, then the referchange, round
        //    by round. The web moves each winner on with the athletes it has at that moment, so a
        //    semifinal sent before the pool finals reached the final without athletes.
        let finished_matches = sqlx::query(
            "SELECT d.id, d.bagan_id, d.round, d.urutan, d.score1, d.score2, d.scorekecil1, d.scorekecil2, \
             d.vr1, d.vr2, d.kata1, d.kata2, d.selesai, d.pemenang, \
             COALESCE(b.pool, 1) AS b_pool, COALESCE(b.nama_bagan, '') AS b_nama, \
             COALESCE(b.round_robin, 0) AS b_rr, b.nomor_tanding_id AS b_nt \
             FROM backend_detailbagan d \
             JOIN backend_bagan b ON d.bagan_id = b.id \
             WHERE b.event_id = ? AND d.selesai = 1 AND COALESCE(d.team, 0) = 0 \
             ORDER BY \
               CASE WHEN UPPER(COALESCE(b.nama_bagan, '')) LIKE '%REFERCHANGE%' OR UPPER(COALESCE(b.nama_bagan, '')) LIKE '%REPECHAGE%' THEN 2 \
                    WHEN COALESCE(b.pool, 1) = 0 OR UPPER(COALESCE(b.nama_bagan, '')) LIKE '%FINAL' THEN 1 ELSE 0 END, \
               d.round ASC, d.bagan_id ASC, d.urutan ASC"
        )
        .bind(local_event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat partai selesai: {}", e))?;

        if finished_matches.is_empty() {
            return Err("Belum ada pertandingan yang selesai untuk disinkronkan.".to_string());
        }

        let mut results_payload = Vec::new();

        for m_row in &finished_matches {
            let d_id: i64 = m_row.get("id");
            let b_id: i64 = m_row.get("bagan_id");
            let round: i32 = m_row.get("round");
            let urutan: i32 = m_row.get("urutan");
            let score1: Option<String> = m_row.get("score1");
            let score2: Option<String> = m_row.get("score2");
            let scorekecil1: Option<String> = m_row.get("scorekecil1");
            let scorekecil2: Option<String> = m_row.get("scorekecil2");
            let vr1: bool = m_row.get("vr1");
            let vr2: bool = m_row.get("vr2");
            let kata1: Option<String> = m_row.get("kata1");
            let kata2: Option<String> = m_row.get("kata2");
            let selesai: bool = m_row.get("selesai");
            let pemenang: Option<String> = m_row.get("pemenang");

            let kode_realtime = format!("{}-{}-{}", event_code, b_id, d_id);

            // Where the winner went: the same rules as the live result (pool winner -> the
            // Final bagan, repechage cascade, round robin has none).
            let b_pool: i32 = m_row.get("b_pool");
            let b_nama: String = m_row.get::<String, _>("b_nama").to_uppercase();
            let b_rr: bool = m_row.get("b_rr");
            let b_nt: Option<i64> = m_row.get("b_nt");
            let is_repechage = b_nama.contains("REFERCHANGE") || b_nama.contains("REPECHAGE");
            let next = {
                let mut conn = pool.acquire().await.map_err(|e| e.to_string())?;
                crate::db::DatabaseManager::live_next_target(
                    &mut conn, b_id, round as i64, urutan as i64, b_rr, is_repechage,
                    crate::db::pool_slot_of(b_pool, &b_nama), b_nt,
                ).await
            };
            let next_kr = next.map(|(nb, nid, _, _)| format!("{}-{}-{}", event_code, nb, nid));
            let target_slot = next.map(|(_, _, slot, _)| slot).unwrap_or(if urutan % 2 == 1 { "atlet1" } else { "atlet2" });

            results_payload.push(serde_json::json!({
                "kode_realtime": kode_realtime,
                "score1": score1,
                "score2": score2,
                "scorekecil1": scorekecil1,
                "scorekecil2": scorekecil2,
                "vr1": vr1,
                "vr2": vr2,
                "kata1": kata1,
                "kata2": kata2,
                "selesai": selesai,
                "pemenang": pemenang,
                "next_kode_realtime": next_kr,
                "target_slot": target_slot,
            }));
        }

        // 2. Champions snapshot
        let champion_rows = sqlx::query(
            "SELECT id, kode, nama_bagan FROM backend_bagan \
             WHERE event_id = ? AND (juara_1_id IS NOT NULL OR juara_2_id IS NOT NULL OR juara_3a_id IS NOT NULL OR juara_3b_id IS NOT NULL)"
        )
        .bind(local_event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat juara bagan: {}", e))?;

        let mut champions_payload = Vec::new();
        for cr in champion_rows {
            let b_kode: Option<String> = cr.get("kode");
            let b_nama: Option<String> = cr.get("nama_bagan");
            let mut champ = serde_json::json!({
                "kode": b_kode.unwrap_or_default(),
                "nama_bagan": b_nama,
            });
            if let Some(podium) = Self::podium_of(pool, cr.get("id")).await? {
                champ.as_object_mut().expect("object").extend(podium);
            }
            champions_payload.push(champ);
        }

        let total_results = results_payload.len();
        let payload = serde_json::json!({
            "event_pk": hosted_event_id,
            "results": results_payload,
            "champions": champions_payload,
        });

        let client = TimedClient::new(std::time::Duration::from_secs(30));

        let target_url =
            format!("{}/api/sync/force-results/", sync_cfg.url.trim_end_matches('/'));
        let auth = Self::auth_headers()?;

        let resp = auth
            .apply(client.post(&target_url))
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Gagal mengirim force sync results: {}", e))?;

        if !resp.status().is_success() {
            return Err(Self::http_error(resp).await);
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Respon server tidak valid: {}", e))?;

        if body.get("status").and_then(|s| s.as_str()) != Some("success") {
            let err = body
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("Server web gagal memproses hasil pertandingan");
            return Err(err.to_string());
        }

        let msg = body
            .get("message")
            .and_then(|m| m.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                format!(
                    "Berhasil menyinkronkan ulang {} hasil pertandingan ke server web.",
                    total_results
                )
            });

        info!("{}", msg);
        Ok(msg)
    }

    /// Toggles is_live_sync_enabled flag for an event
    pub async fn toggle_live_sync(
        pool: &Pool<Sqlite>,
        local_event_id: i64,
        enabled: bool,
    ) -> Result<bool, String> {
        sqlx::query("UPDATE backend_event SET is_live_sync_enabled = ? WHERE id = ?")
            .bind(enabled)
            .bind(local_event_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal memperbarui status live sync: {}", e))?;

        info!(
            "Live sync untuk event #{} diubah ke: {}",
            local_event_id, enabled
        );
        Ok(enabled)
    }

    /// Ensures backend_syncqueue table exists in SQLite database
    pub async fn ensure_sync_queue_table(pool: &Pool<Sqlite>) -> Result<(), String> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS backend_syncqueue (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id INTEGER REFERENCES backend_event(id) ON DELETE CASCADE,
                endpoint VARCHAR(100) NOT NULL,
                payload TEXT NOT NULL,
                status VARCHAR(20) NOT NULL DEFAULT 'pending',
                retry_count INTEGER NOT NULL DEFAULT 0,
                last_error TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )"
        )
        .execute(pool)
        .await
        .map_err(|e| format!("Gagal membuat tabel backend_syncqueue: {}", e))?;

        let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_syncqueue_status ON backend_syncqueue(status)")
            .execute(pool)
            .await;
        let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_syncqueue_event ON backend_syncqueue(event_id)")
            .execute(pool)
            .await;

        Ok(())
    }

    /// Fetches sync queue statistics
    pub async fn get_queue_stats(
        pool: &Pool<Sqlite>,
        event_id: Option<i64>,
    ) -> Result<SyncQueueStats, String> {
        let _ = Self::ensure_sync_queue_table(pool).await;

        let (total_count, failed_count, pending_count, synced_count): (i64, i64, i64, i64) = if let Some(eid) = event_id {
            let total = sqlx::query_scalar("SELECT COUNT(*) FROM backend_syncqueue WHERE event_id = ?")
                .bind(eid)
                .fetch_one(pool)
                .await
                .unwrap_or(0);
            let failed = sqlx::query_scalar("SELECT COUNT(*) FROM backend_syncqueue WHERE event_id = ? AND status IN ('failed', 'abandoned')")
                .bind(eid)
                .fetch_one(pool)
                .await
                .unwrap_or(0);
            let pending = sqlx::query_scalar("SELECT COUNT(*) FROM backend_syncqueue WHERE event_id = ? AND (status = 'pending' OR status = 'processing')")
                .bind(eid)
                .fetch_one(pool)
                .await
                .unwrap_or(0);
            let synced = sqlx::query_scalar("SELECT COUNT(*) FROM backend_syncqueue WHERE event_id = ? AND status = 'synced'")
                .bind(eid)
                .fetch_one(pool)
                .await
                .unwrap_or(0);
            (total, failed, pending, synced)
        } else {
            let total = sqlx::query_scalar("SELECT COUNT(*) FROM backend_syncqueue")
                .fetch_one(pool)
                .await
                .unwrap_or(0);
            let failed = sqlx::query_scalar("SELECT COUNT(*) FROM backend_syncqueue WHERE status IN ('failed', 'abandoned')")
                .fetch_one(pool)
                .await
                .unwrap_or(0);
            let pending = sqlx::query_scalar("SELECT COUNT(*) FROM backend_syncqueue WHERE status = 'pending' OR status = 'processing'")
                .fetch_one(pool)
                .await
                .unwrap_or(0);
            let synced = sqlx::query_scalar("SELECT COUNT(*) FROM backend_syncqueue WHERE status = 'synced'")
                .fetch_one(pool)
                .await
                .unwrap_or(0);
            (total, failed, pending, synced)
        };

        Ok(SyncQueueStats {
            total_count,
            failed_count,
            pending_count,
            synced_count,
        })
    }

    /// Fetches sync queue items with optional status filtering
    pub async fn get_queue_items(
        pool: &Pool<Sqlite>,
        event_id: Option<i64>,
        status_filter: Option<String>,
        limit: i64,
    ) -> Result<Vec<SyncQueueEntry>, String> {
        let _ = Self::ensure_sync_queue_table(pool).await;

        let status_clean = status_filter.unwrap_or_else(|| "all".to_string());

        let rows = if status_clean == "failed" {
            if let Some(eid) = event_id {
                sqlx::query("SELECT id, event_id, endpoint, payload, status, retry_count, last_error, created_at, updated_at FROM backend_syncqueue WHERE event_id = ? AND status IN ('failed', 'abandoned') ORDER BY id ASC LIMIT ?")
                    .bind(eid).bind(limit).fetch_all(pool).await
            } else {
                sqlx::query("SELECT id, event_id, endpoint, payload, status, retry_count, last_error, created_at, updated_at FROM backend_syncqueue WHERE status IN ('failed', 'abandoned') ORDER BY id ASC LIMIT ?")
                    .bind(limit).fetch_all(pool).await
            }
        } else if status_clean == "pending" {
            if let Some(eid) = event_id {
                sqlx::query("SELECT id, event_id, endpoint, payload, status, retry_count, last_error, created_at, updated_at FROM backend_syncqueue WHERE event_id = ? AND (status = 'pending' OR status = 'processing') ORDER BY id ASC LIMIT ?")
                    .bind(eid).bind(limit).fetch_all(pool).await
            } else {
                sqlx::query("SELECT id, event_id, endpoint, payload, status, retry_count, last_error, created_at, updated_at FROM backend_syncqueue WHERE (status = 'pending' OR status = 'processing') ORDER BY id ASC LIMIT ?")
                    .bind(limit).fetch_all(pool).await
            }
        } else if status_clean == "synced" {
            if let Some(eid) = event_id {
                sqlx::query("SELECT id, event_id, endpoint, payload, status, retry_count, last_error, created_at, updated_at FROM backend_syncqueue WHERE event_id = ? AND status = 'synced' ORDER BY id DESC LIMIT ?")
                    .bind(eid).bind(limit).fetch_all(pool).await
            } else {
                sqlx::query("SELECT id, event_id, endpoint, payload, status, retry_count, last_error, created_at, updated_at FROM backend_syncqueue WHERE status = 'synced' ORDER BY id DESC LIMIT ?")
                    .bind(limit).fetch_all(pool).await
            }
        } else {
            if let Some(eid) = event_id {
                sqlx::query("SELECT id, event_id, endpoint, payload, status, retry_count, last_error, created_at, updated_at FROM backend_syncqueue WHERE event_id = ? ORDER BY id ASC LIMIT ?")
                    .bind(eid).bind(limit).fetch_all(pool).await
            } else {
                sqlx::query("SELECT id, event_id, endpoint, payload, status, retry_count, last_error, created_at, updated_at FROM backend_syncqueue ORDER BY id ASC LIMIT ?")
                    .bind(limit).fetch_all(pool).await
            }
        }.map_err(|e| format!("Gagal memuat antrean sync: {}", e))?;

        let mut list = Vec::new();
        for r in rows {
            let id: i64 = r.get("id");
            let eid: Option<i64> = r.get("event_id");
            let endpoint: String = r.get("endpoint");
            let payload: String = r.get("payload");
            let status: String = r.get("status");
            let retry_count: i32 = r.get("retry_count");
            let last_error: Option<String> = r.get("last_error");
            let created_at: Option<String> = r.get("created_at");
            let updated_at: Option<String> = r.get("updated_at");

            // Extract display helpers from payload JSON
            let mut kode_realtime = None;
            let mut ring = None;
            let mut round = None;
            let mut urutan = None;
            let mut score = None;
            let mut winner = None;

            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&payload) {
                kode_realtime = v.get("kode_realtime").and_then(|s| s.as_str()).map(|s| s.to_string());
                // Payloads carry the tatami as `ring_number` (a string); older rows used `ring`.
                ring = v.get("ring_number").or_else(|| v.get("ring")).and_then(|s| {
                    s.as_i64().or_else(|| s.as_str().and_then(|t| t.trim().parse::<i64>().ok()))
                }).map(|i| i as i32);
                round = v.get("round").and_then(|s| s.as_i64()).map(|i| i as i32);
                urutan = v.get("urutan").and_then(|s| s.as_i64()).map(|i| i as i32);
                
                let s1 = v.get("score1").and_then(|s| s.as_str());
                let s2 = v.get("score2").and_then(|s| s.as_str());
                if let (Some(sc1), Some(sc2)) = (s1, s2) {
                    score = Some(format!("{} - {}", sc1, sc2));
                }

                winner = v.get("pemenang").and_then(|p| p.as_str()).map(|w| {
                    match w {
                        "1" => "Aka (Merah)".to_string(),
                        "2" => "Ao (Biru)".to_string(),
                        "3" => "Draw".to_string(),
                        other => other.to_string(),
                    }
                });
            }

            list.push(SyncQueueEntry {
                id,
                event_id: eid,
                endpoint,
                payload,
                status,
                retry_count,
                last_error,
                created_at,
                updated_at,
                kode_realtime,
                ring,
                round,
                urutan,
                score,
                winner,
            });
        }

        Ok(list)
    }

    /// Retries a single queue item
    pub async fn retry_single_item(
        pool: &Pool<Sqlite>,
        db_path: Option<&PathBuf>,
        item_id: i64,
    ) -> Result<bool, String> {
        let row = sqlx::query(
            "SELECT id, endpoint, payload, retry_count FROM backend_syncqueue WHERE id = ?"
        )
        .bind(item_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

        let Some(r) = row else {
            return Err(format!("Item #{} tidak ditemukan", item_id));
        };

        let endpoint: String = r.get("endpoint");
        let payload_str: String = r.get("payload");
        let retry_count: i32 = r.get("retry_count");

        let sync_cfg = Self::get_sync_config(db_path);
        let url = format!("{}/{}", sync_cfg.url.trim_end_matches('/'), endpoint.trim_start_matches('/'));
        Self::clear_auth_block();
        let auth = Self::auth_headers()?;

        // Claim it, so the background worker does not send the same row at the same time.
        let claimed = sqlx::query(
            "UPDATE backend_syncqueue SET status = 'processing', updated_at = CURRENT_TIMESTAMP
             WHERE id = ? AND status IN ('pending', 'failed', 'abandoned')"
        )
        .bind(item_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected() == 1;
        if !claimed {
            return Err(format!("Item #{} sedang dikirim atau sudah terkirim", item_id));
        }

        let client = TimedClient::new(std::time::Duration::from_secs(5));

        match auth
            .apply(client.post(&url))
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(payload_str)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                sqlx::query(
                    "UPDATE backend_syncqueue SET status = 'synced', last_error = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
                )
                .bind(item_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
                Ok(true)
            }
            Ok(resp) => {
                let refused = matches!(resp.status().as_u16(), 401 | 403);
                let err_msg = Self::http_error(resp).await;
                if refused {
                    // Same rule as the worker: the web refused this computer, pause sending.
                    Self::set_auth_block(&err_msg);
                }
                sqlx::query(
                    "UPDATE backend_syncqueue SET status = 'failed', retry_count = ?, last_error = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
                )
                .bind(retry_count + 1)
                .bind(&err_msg)
                .bind(item_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
                Err(err_msg)
            }
            Err(e) => {
                let err_msg = e.to_string();
                sqlx::query(
                    "UPDATE backend_syncqueue SET status = 'failed', retry_count = ?, last_error = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
                )
                .bind(retry_count + 1)
                .bind(&err_msg)
                .bind(item_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
                Err(err_msg)
            }
        }
    }

    /// Retries all failed/pending items
    pub async fn retry_all_failed(
        pool: &Pool<Sqlite>,
        db_path: Option<&PathBuf>,
        event_id: Option<i64>,
    ) -> Result<usize, String> {
        let items: Vec<i64> = if let Some(eid) = event_id {
            sqlx::query_scalar(
                "SELECT id FROM backend_syncqueue WHERE event_id = ? AND status IN ('failed', 'abandoned') ORDER BY id ASC"
            )
            .bind(eid)
            .fetch_all(pool)
            .await
            .unwrap_or_default()
        } else {
            sqlx::query_scalar(
                "SELECT id FROM backend_syncqueue WHERE status IN ('failed', 'abandoned') ORDER BY id ASC"
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default()
        };

        let mut success_count = 0;
        for id in items {
            if Self::retry_single_item(pool, db_path, id).await.is_ok() {
                success_count += 1;
            } else if AUTH_BLOCKED_UNTIL.load(Ordering::Relaxed) > now_secs() {
                // Refused (401/403): the rest would be refused too, stop instead of hammering the web.
                break;
            }
        }

        Ok(success_count)
    }

    /// Marks a queue item as synced (skip)
    pub async fn mark_item_synced(pool: &Pool<Sqlite>, item_id: i64) -> Result<(), String> {
        sqlx::query("UPDATE backend_syncqueue SET status = 'synced', updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(item_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal memperbarui status: {}", e))?;
        Ok(())
    }

    /// Deletes a queue item
    pub async fn delete_queue_item(pool: &Pool<Sqlite>, item_id: i64) -> Result<(), String> {
        sqlx::query("DELETE FROM backend_syncqueue WHERE id = ?")
            .bind(item_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal menghapus item: {}", e))?;
        Ok(())
    }

    /// Clears all synced items
    pub async fn clear_synced_items(pool: &Pool<Sqlite>, event_id: Option<i64>) -> Result<u64, String> {
        let res = if let Some(eid) = event_id {
            sqlx::query("DELETE FROM backend_syncqueue WHERE event_id = ? AND status = 'synced'")
                .bind(eid)
                .execute(pool)
                .await
        } else {
            sqlx::query("DELETE FROM backend_syncqueue WHERE status = 'synced'")
                .execute(pool)
                .await
        }.map_err(|e| format!("Gagal membersihkan data tersinkron: {}", e))?;

        Ok(res.rows_affected())
    }

    /// Processes the sync queue in strict FIFO order for active events where live sync is enabled.
    /// Returns (synced_count, failed_count).
    pub async fn process_sync_queue(
        pool: &Pool<Sqlite>,
        db_path: Option<&PathBuf>,
    ) -> Result<(usize, usize), String> {
        // One sender at a time: the 12 s worker and the task spawned after every save would
        // otherwise pick the same row and send it twice, or send row 2 before row 1 is done.
        // A caller that finds a run in progress asks it to look again before it stops.
        let run = queue_run_for(db_path);
        let Ok(_guard) = run.lock.try_lock() else {
            run.rerun.store(true, Ordering::SeqCst);
            return Ok((0, 0));
        };
        let mut total = (0usize, 0usize);
        loop {
            run.rerun.store(false, Ordering::SeqCst);
            let (s, f) = Self::process_sync_queue_once(pool, db_path).await?;
            total.0 += s;
            total.1 += f;
            // Stop when nobody asked for another look, or the queue just paused/halted
            // (a rerun right away would only repeat the same refused/offline request).
            if !run.rerun.load(Ordering::SeqCst) || f > 0 {
                return Ok(total);
            }
        }
    }

    async fn process_sync_queue_once(
        pool: &Pool<Sqlite>,
        db_path: Option<&PathBuf>,
    ) -> Result<(usize, usize), String> {
        let mut synced_count = 0;
        let mut failed_count = 0;

        // An item left in 'processing' (app closed or crashed mid-request) would never be
        // picked up again: put it back in the queue after a minute.
        let _ = sqlx::query(
            "UPDATE backend_syncqueue SET status = 'failed', updated_at = CURRENT_TIMESTAMP
             WHERE status = 'processing' AND updated_at < datetime('now', '-60 seconds')"
        )
        .execute(pool)
        .await;

        if AUTH_BLOCKED_UNTIL.load(Ordering::Relaxed) > now_secs() {
            return Ok((0, 0));
        }
        // No license and no legacy token: keep everything queued; the Sinkronisasi web channel says why.
        let Ok(auth) = Self::auth_headers() else {
            return Ok((0, 0));
        };
        let base_url = Self::get_sync_config(db_path).url.trim_end_matches('/').to_string();
        let client = TimedClient::new(std::time::Duration::from_secs(6));

        loop {
            // Find oldest active pending or failed item whose event has is_live_sync_enabled = 1 (or no event)
            let row = sqlx::query(
                r#"
                SELECT sq.id, sq.endpoint, sq.payload, sq.retry_count, sq.event_id
                FROM backend_syncqueue sq
                LEFT JOIN backend_event ev ON sq.event_id = ev.id
                WHERE sq.status IN ('pending', 'failed')
                  AND (sq.event_id IS NULL OR ev.is_live_sync_enabled = 1)
                ORDER BY sq.id ASC
                LIMIT 1
                "#
            )
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;

            let Some(r) = row else {
                break; // No more pending or failed items
            };

            let item_id: i64 = r.get("id");
            let endpoint: String = r.get("endpoint");
            let payload_str: String = r.get("payload");
            let retry_count: i32 = r.get("retry_count");

            // Claim the row (a manual "Coba lagi" may have taken it in the meantime).
            let claimed = sqlx::query(
                "UPDATE backend_syncqueue SET status = 'processing', updated_at = CURRENT_TIMESTAMP
                 WHERE id = ? AND status IN ('pending', 'failed')"
            )
            .bind(item_id)
            .execute(pool)
            .await
            .map(|r| r.rows_affected() == 1)
            .unwrap_or(false);
            if !claimed {
                continue;
            }

            let url = format!("{}/{}", base_url, endpoint.trim_start_matches('/'));

            // Rows are queued with the local event: the web needs its own event id. Live bout
            // messages carry it too, because `kode_realtime` is only unique within one web event.
            let payload_str = if Self::needs_web_event_id(&endpoint) {
                let local_event: Option<i64> = r.get("event_id");
                match (serde_json::from_str::<serde_json::Value>(&payload_str), local_event) {
                    (Ok(mut v), Some(local)) if v.get("event_id").is_none() => {
                        let mode = Self::get_sync_config(db_path).mode;
                        if let Some(hosted) = Self::get_event_mapping(db_path, local, None, &mode).hosted_event_id {
                            v["event_id"] = serde_json::json!(hosted);
                        }
                        v.to_string()
                    }
                    _ => payload_str,
                }
            } else {
                payload_str
            };

            // The payload is stored as JSON already: send it as is (no parse + re-serialize).
            match auth
                .apply(client.post(&url))
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(payload_str)
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    let _ = sqlx::query(
                        "UPDATE backend_syncqueue SET status = 'synced', last_error = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
                    )
                    .bind(item_id)
                    .execute(pool)
                    .await;
                    synced_count += 1;
                    info!("SyncQueue #{} to {} synced successfully", item_id, endpoint);
                    // Strict FIFO: continue immediately to next item
                    continue;
                }
                Ok(resp) => {
                    let status = resp.status();
                    let err_msg = Self::http_error(resp).await;
                    let new_retry = retry_count + 1;

                    // 401/403: the web refused this license (not active, or the event is not granted).
                    // Every following item would be refused too, so keep them all queued and pause.
                    if status.as_u16() == 401 || status.as_u16() == 403 {
                        let _ = sqlx::query(
                            "UPDATE backend_syncqueue SET status = 'failed', last_error = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
                        )
                        .bind(&err_msg)
                        .bind(item_id)
                        .execute(pool)
                        .await;
                        Self::set_auth_block(&err_msg);
                        failed_count += 1;
                        warn!("SyncQueue #{} to {} refused ({}); queue paused: {}", item_id, endpoint, status, err_msg);
                        break;
                    }

                    // The web is there but cannot answer right now (5xx while it restarts or is
                    // deployed, 408/429 under load): like no connection, never a reason to give
                    // up. Keep the row and stop here (strict FIFO) until the next tick.
                    let code = status.as_u16();
                    if status.is_server_error() || code == 408 || code == 429 {
                        let _ = sqlx::query(
                            "UPDATE backend_syncqueue SET status = 'failed', retry_count = ?, last_error = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
                        )
                        .bind(new_retry)
                        .bind(&err_msg)
                        .bind(item_id)
                        .execute(pool)
                        .await;
                        failed_count += 1;
                        warn!("SyncQueue #{} to {} returned {}, retried later: {}", item_id, endpoint, status, err_msg);
                        break;
                    }

                    // The web refused the message itself (400 bad payload, 404 bout not on the
                    // web, other 4xx after 5 tries): skip it so later matches are not blocked.
                    // It stays visible under "Gagal" and is sent again by "Kirim ulang".
                    let is_unrecoverable = code == 400 || code == 404 || new_retry >= 5;

                    let final_status = if is_unrecoverable { "abandoned" } else { "failed" };
                    let _ = sqlx::query(
                        "UPDATE backend_syncqueue SET status = ?, retry_count = ?, last_error = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
                    )
                    .bind(final_status)
                    .bind(new_retry)
                    .bind(&err_msg)
                    .bind(item_id)
                    .execute(pool)
                    .await;

                    failed_count += 1;
                    warn!("SyncQueue #{} to {} returned {}: {}", item_id, endpoint, status, err_msg);
                    if is_unrecoverable {
                        // Unrecoverable: skip to unblock other matches
                        continue;
                    } else {
                        // Temporary error: halt queue to preserve FIFO
                        break;
                    }
                }
                Err(e) => {
                    // No connection (venue internet down, web server unreachable) is never a
                    // reason to give up: the item stays queued and is sent when the link is back.
                    let err_msg = e.to_string();
                    let new_retry = retry_count + 1;
                    let final_status = "failed";

                    let _ = sqlx::query(
                        "UPDATE backend_syncqueue SET status = ?, retry_count = ?, last_error = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
                    )
                    .bind(final_status)
                    .bind(new_retry)
                    .bind(&err_msg)
                    .bind(item_id)
                    .execute(pool)
                    .await;

                    failed_count += 1;
                    warn!("SyncQueue #{} to {} connection failed: {}", item_id, endpoint, err_msg);
                    // Offline: halt until next tick (strict FIFO)
                    break;
                }
            }
        }

        Ok((synced_count, failed_count))
    }

    /// Spawns the background worker that retries pending/failed items every 12 seconds.
    /// It is a task on the app's async runtime: a dedicated OS thread with its own Tokio
    /// runtime cost a thread stack and a second runtime just to sleep, and made the shared
    /// HTTP client's connections live on two runtimes.
    pub fn spawn_sync_worker(
        db: Arc<Mutex<crate::db::DatabaseManager>>,
        app_handle: Option<tauri::AppHandle>,
    ) {
        tauri::async_runtime::spawn(async move {
            info!("Ambrilindo Live Sync Background Daemon running (12s interval)");
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(12)).await;

                let (pool_opt, db_path_opt) = {
                    let db_guard = db.lock().await;
                    (db_guard.get_pool_cloned(), db_guard.get_active_db_path_cloned())
                };

                if let Some(pool) = pool_opt {
                    if let Ok((synced, failed)) = Self::process_sync_queue(&pool, db_path_opt.as_ref()).await {
                        if (synced > 0 || failed > 0) && app_handle.is_some() {
                            if let Some(app) = &app_handle {
                                let _ = app.emit("sync_queue_changed", serde_json::json!({
                                    "synced": synced,
                                    "failed": failed
                                }));
                            }
                        }
                    }
                }
            }
        });
    }

    /// Clears all queue items
    pub async fn clear_all_queue(pool: &Pool<Sqlite>, event_id: Option<i64>) -> Result<u64, String> {
        let res = if let Some(eid) = event_id {
            sqlx::query("DELETE FROM backend_syncqueue WHERE event_id = ?")
                .bind(eid)
                .execute(pool)
                .await
        } else {
            sqlx::query("DELETE FROM backend_syncqueue")
                .execute(pool)
                .await
        }.map_err(|e| format!("Gagal mengosongkan antrean: {}", e))?;

        Ok(res.rows_affected())
    }
}



#[cfg(test)]
mod scope_tests {
    use super::SyncService;
    use serde_json::json;

    #[test]
    fn mapping_ignores_legacy_keys() {
        let dir = std::env::temp_dir().join(format!("ambr_map_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("db.sqlite3");
        std::fs::write(
            dir.join("event_mapping.json"),
            json!({ "local_5": { "hosted_event_id": 99, "hosted_event_name": "Lama" },
                    "local_6_custom": { "hosted_event_id": 7, "hosted_event_name": "Baru" } }).to_string(),
        )
        .unwrap();
        assert_eq!(SyncService::get_event_mapping(Some(&db), 5, None, "custom").hosted_event_id, None);
        assert_eq!(SyncService::get_event_mapping(Some(&db), 6, None, "custom").hosted_event_id, Some(7));
        assert_eq!(SyncService::get_event_mapping(Some(&db), 6, None, "production").hosted_event_id, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hosted_events_read_web_field_names() {
        let body = json!({ "status": "success", "events": [
            { "id": 3, "nama_event": "Kejurda", "tempat_event": "GOR Pontianak", "tanggal_event": "2026-12-01" },
            { "id": 4, "nama_event": "Lama", "tempat": "Aula", "tanggal_mulai": "2026-01-02" },
            { "id": 5, "nama_event": "", "tempat_event": "", "tanggal_event": "" },
            { "nama_event": "tanpa id" },
        ]});
        let ev = SyncService::parse_hosted_events(&body);
        assert_eq!(ev.len(), 3);
        assert_eq!((ev[0].tempat.as_deref(), ev[0].tanggal_mulai.as_deref()), (Some("GOR Pontianak"), Some("2026-12-01")));
        assert_eq!((ev[1].tempat.as_deref(), ev[1].tanggal_mulai.as_deref()), (Some("Aula"), Some("2026-01-02")));
        assert_eq!((ev[2].nama_event.as_str(), ev[2].tempat.as_deref()), ("Event Tanpa Nama", None));
    }

    #[test]
    fn live_messages_get_the_web_event_id() {
        for e in ["api/status/", "api/result/", "api/reopen/", "api/final-result/", "api/sync/tatami-status/"] {
            assert!(SyncService::needs_web_event_id(e), "{e}");
        }
        assert!(!SyncService::needs_web_event_id("api/sync/push-bagan/"));
    }
}
