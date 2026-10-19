use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tracing::info;

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

    /// Read hosted API token from env with automatic .env search and dev fallback
    pub fn get_api_token() -> String {
        if let Ok(token) = std::env::var("HOSTED_API_TOKEN").or_else(|_| std::env::var("API_TOKEN")) {
            if !token.trim().is_empty() {
                return token.trim().to_string();
            }
        }

        // Search for .env in current and parent/adjacent directories
        let candidates = [
            PathBuf::from(".env"),
            PathBuf::from("../.env"),
            PathBuf::from("../../.env"),
            PathBuf::from("tournament_data/.env"),
            PathBuf::from("../ambrilindo/ambrilindo/.env"),
            PathBuf::from("../../ambrilindo/ambrilindo/.env"),
            PathBuf::from("../ambrilindo_web/ambrilindo_web_pub/.env"),
            PathBuf::from("../../ambrilindo_web/ambrilindo_web_pub/.env"),
        ];

        for path in &candidates {
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(path) {
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if trimmed.starts_with('#') || trimmed.is_empty() {
                            continue;
                        }
                        if let Some((key, val)) = trimmed.split_once('=') {
                            let k = key.trim();
                            if k == "HOSTED_API_TOKEN" || k == "API_TOKEN" {
                                let v = val.trim().trim_matches('"').trim_matches('\'').trim();
                                if !v.is_empty() {
                                    return v.to_string();
                                }
                            }
                        }
                    }
                }
            }
        }

        "ambrilindo-secret-api-token-dev".to_string()
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
        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(3500))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                return PingResult {
                    success: false,
                    status_code: None,
                    latency_ms: 0,
                    message: format!("Gagal inisialisasi HTTP client: {}", e),
                }
            }
        };

        let target_url = format!("{}/api/sync/events/", url.trim_end_matches('/'));
        let token = Self::get_api_token();
        let start = Instant::now();

        match client
            .get(&target_url)
            .header("Authorization", format!("Bearer {}", token))
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
                        message: format!("Server merespons kode HTTP {}", status.as_u16()),
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
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(5000))
            .build()
            .map_err(|e| e.to_string())?;

        let target_url = format!("{}/api/sync/events/", url.trim_end_matches('/'));
        let token = Self::get_api_token();

        let resp = client
            .get(&target_url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| format!("Gagal memuat event dari web server: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("Server mengembalikan status HTTP {}", resp.status()));
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

        let mut events = Vec::new();
        if let Some(arr) = body.get("events").and_then(|e| e.as_array()) {
            for item in arr {
                if let Some(id) = item.get("id").and_then(|i| i.as_i64()) {
                    let nama = item
                        .get("nama_event")
                        .and_then(|n| n.as_str())
                        .unwrap_or("Event Tanpa Nama")
                        .to_string();
                    let tgl_m = item
                        .get("tanggal_mulai")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string());
                    let tgl_s = item
                        .get("tanggal_selesai")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string());
                    let tempat = item
                        .get("tempat")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string());
                    let status = item
                        .get("status")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string());

                    events.push(HostedEventOption {
                        id,
                        nama_event: nama,
                        tanggal_mulai: tgl_m,
                        tanggal_selesai: tgl_s,
                        tempat,
                        status,
                    });
                }
            }
        }

        Ok(events)
    }

    /// Reads event mapping configuration from event_mapping.json
    pub fn get_event_mapping(
        db_path: Option<&PathBuf>,
        local_event_id: i64,
        local_event_name: Option<&str>,
        target_mode: &str,
    ) -> EventMapping {
        let mapping_file = Self::get_config_dir(db_path).join("event_mapping.json");
        let fallback_mapping_file =
            Path::new("../ambrilindo/ambrilindo/database/event_mapping.json");

        let path_to_read = if mapping_file.exists() {
            Some(mapping_file)
        } else if fallback_mapping_file.exists() {
            Some(fallback_mapping_file.to_path_buf())
        } else {
            None
        };

        let mut hosted_id = None;
        let mut hosted_name = None;

        if let Some(p) = path_to_read {
            if let Ok(content) = std::fs::read_to_string(&p) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                    let key = format!("local_{}_{}", local_event_id, target_mode);
                    let fallback_key = format!("local_{}", local_event_id);

                    let entry = val.get(&key).or_else(|| val.get(&fallback_key));
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

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;

        let target_url = format!(
            "{}/api/sync/export-atlet/{}/",
            sync_cfg.url.trim_end_matches('/'),
            hosted_event_id
        );
        let token = Self::get_api_token();

        let resp = client
            .get(&target_url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| format!("Gagal koneksi ke server web saat menarik atlet: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("Server web mengembalikan status HTTP {}", resp.status()));
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
                        .fetch_optional(pool)
                        .await
                        .map_err(|e| e.to_string())?;

                        if existing.is_none() {
                            sqlx::query(
                                "INSERT INTO backend_perguruan (event_id, nama_perguruan) VALUES (?, ?)"
                            )
                            .bind(local_event_id)
                            .bind(&clean)
                            .execute(pool)
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
                    .fetch_optional(pool)
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
                            .execute(pool)
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
                        .fetch_optional(pool)
                        .await
                        .map_err(|e| e.to_string())?;

                        if existing.is_none() {
                            sqlx::query(
                                "INSERT INTO backend_utusan (event_id, nama_utusan, logo) VALUES (?, ?, NULL)"
                            )
                            .bind(local_event_id)
                            .bind(&clean)
                            .execute(pool)
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
                    .fetch_optional(pool)
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
                                .execute(pool)
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
                            .execute(pool)
                            .await
                            .map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
        }

        // 4. Sync Atlet (Idempotent update or insert)
        if let Some(atlets_data) = body.get("atlets").and_then(|a| a.as_array()) {
            for a_item in atlets_data {
                let mut nama = a_item
                    .get("nama_atlet")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let perguruan_name = a_item
                    .get("perguruan")
                    .and_then(|p| p.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_uppercase();
                let utusan_name = a_item
                    .get("utusan")
                    .and_then(|u| u.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_uppercase();
                let nt_name = a_item
                    .get("nomor_tanding")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .trim();
                let nik = a_item
                    .get("nik")
                    .and_then(|n| n.as_str())
                    .map(|s| s.trim().to_string());
                let kode_atlet = a_item
                    .get("kode_atlet")
                    .and_then(|k| k.as_str())
                    .map(|s| s.trim().to_string());
                let public_detail_id = a_item.get("public_detail_id").and_then(|p| p.as_i64());
                let is_priority = a_item
                    .get("is_priority")
                    .and_then(|ip| ip.as_bool())
                    .unwrap_or(false);

                if nama.is_empty() {
                    nama = if let Some(ref k) = kode_atlet {
                        format!("ATLET {}", k)
                    } else {
                        "-".to_string()
                    };
                }

                if nt_name.is_empty() {
                    continue;
                }

                // Resolve Perguruan ID
                let perguruan_id: Option<i64> = if !perguruan_name.is_empty() {
                    sqlx::query_scalar(
                        "SELECT id FROM backend_perguruan WHERE event_id = ? AND UPPER(nama_perguruan) = ?"
                    )
                    .bind(local_event_id)
                    .bind(&perguruan_name)
                    .fetch_optional(pool)
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
                    .fetch_optional(pool)
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
                .bind(nt_name)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?;

                let Some(nomor_tanding_id) = nt_id else {
                    continue;
                };

                let additional_code_val = public_detail_id.map(|id| id.to_string());

                // Find existing athlete:
                // a. By additional_code
                let mut existing_atlet_id: Option<i64> = None;
                if let Some(ref add_code) = additional_code_val {
                    existing_atlet_id = sqlx::query_scalar(
                        "SELECT id FROM backend_atlet WHERE event_id = ? AND nomor_tanding_id = ? AND additional_code = ?"
                    )
                    .bind(local_event_id)
                    .bind(nomor_tanding_id)
                    .bind(add_code)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| e.to_string())?;
                }

                // b. By kode_atlet
                if existing_atlet_id.is_none() {
                    if let Some(ref k_code) = kode_atlet {
                        existing_atlet_id = sqlx::query_scalar(
                            "SELECT id FROM backend_atlet WHERE event_id = ? AND nomor_tanding_id = ? AND kode_atlet = ?"
                        )
                        .bind(local_event_id)
                        .bind(nomor_tanding_id)
                        .bind(k_code)
                        .fetch_optional(pool)
                        .await
                        .map_err(|e| e.to_string())?;
                    }
                }

                // c. By nama + utusan
                if existing_atlet_id.is_none() && nama != "-" && !nama.starts_with("ATLET ") {
                    if let Some(u_id) = utusan_id {
                        existing_atlet_id = sqlx::query_scalar(
                            "SELECT id FROM backend_atlet WHERE event_id = ? AND nomor_tanding_id = ? AND LOWER(nama_atlet) = LOWER(?) AND utusan_id = ?"
                        )
                        .bind(local_event_id)
                        .bind(nomor_tanding_id)
                        .bind(&nama)
                        .bind(u_id)
                        .fetch_optional(pool)
                        .await
                        .map_err(|e| e.to_string())?;
                    }
                    if existing_atlet_id.is_none() {
                        existing_atlet_id = sqlx::query_scalar(
                            "SELECT id FROM backend_atlet WHERE event_id = ? AND nomor_tanding_id = ? AND LOWER(nama_atlet) = LOWER(?)"
                        )
                        .bind(local_event_id)
                        .bind(nomor_tanding_id)
                        .bind(&nama)
                        .fetch_optional(pool)
                        .await
                        .map_err(|e| e.to_string())?;
                    }
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
                        .execute(pool)
                        .await
                        .map_err(|e| e.to_string())?;

                        updated_count += 1;
                    }
                    None => {
                        sqlx::query(
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
                        .execute(pool)
                        .await
                        .map_err(|e| e.to_string())?;

                        created_count += 1;
                    }
                }
            }
        }

        // 5. Download Utusan Logos
        let mut logo_count = 0;
        let media_dir = Self::get_media_dir(db_path).join("logo_utusan");
        let _ = std::fs::create_dir_all(&media_dir);

        let img_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();

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
                 nt.nama_nomor_tanding, \
                 j1.nama_atlet as j1_nama, j1.kode_atlet as j1_kode, j1.additional_code as j1_add, \
                 j2.nama_atlet as j2_nama, j2.kode_atlet as j2_kode, j2.additional_code as j2_add, \
                 j3a.nama_atlet as j3a_nama, j3a.kode_atlet as j3a_kode, j3a.additional_code as j3a_add, \
                 j3b.nama_atlet as j3b_nama, j3b.kode_atlet as j3b_kode, j3b.additional_code as j3b_add \
                 FROM backend_bagan b \
                 LEFT JOIN backend_nomortanding nt ON b.nomor_tanding_id = nt.id \
                 LEFT JOIN backend_atlet j1 ON b.juara_1_id = j1.id \
                 LEFT JOIN backend_atlet j2 ON b.juara_2_id = j2.id \
                 LEFT JOIN backend_atlet j3a ON b.juara_3a_id = j3a.id \
                 LEFT JOIN backend_atlet j3b ON b.juara_3b_id = j3b.id \
                 WHERE b.event_id = ? AND b.id IN ({}) ORDER BY b.id ASC",
                placeholders
            )
        } else {
            "SELECT b.id, b.nama_bagan, b.tipe_tanding, b.pool, b.kode, b.is_bob, \
             nt.nama_nomor_tanding, \
             j1.nama_atlet as j1_nama, j1.kode_atlet as j1_kode, j1.additional_code as j1_add, \
             j2.nama_atlet as j2_nama, j2.kode_atlet as j2_kode, j2.additional_code as j2_add, \
             j3a.nama_atlet as j3a_nama, j3a.kode_atlet as j3a_kode, j3a.additional_code as j3a_add, \
             j3b.nama_atlet as j3b_nama, j3b.kode_atlet as j3b_kode, j3b.additional_code as j3b_add \
             FROM backend_bagan b \
             LEFT JOIN backend_nomortanding nt ON b.nomor_tanding_id = nt.id \
             LEFT JOIN backend_atlet j1 ON b.juara_1_id = j1.id \
             LEFT JOIN backend_atlet j2 ON b.juara_2_id = j2.id \
             LEFT JOIN backend_atlet j3a ON b.juara_3a_id = j3a.id \
             LEFT JOIN backend_atlet j3b ON b.juara_3b_id = j3b.id \
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

            let j1_nama: Option<String> = b_row.get("j1_nama");
            let j1_kode: Option<String> = b_row.get("j1_kode");
            let j1_add: Option<String> = b_row.get("j1_add");

            let j2_nama: Option<String> = b_row.get("j2_nama");
            let j2_kode: Option<String> = b_row.get("j2_kode");
            let j2_add: Option<String> = b_row.get("j2_add");

            let j3a_nama: Option<String> = b_row.get("j3a_nama");
            let j3a_kode: Option<String> = b_row.get("j3a_kode");
            let j3a_add: Option<String> = b_row.get("j3a_add");

            let j3b_nama: Option<String> = b_row.get("j3b_nama");
            let j3b_kode: Option<String> = b_row.get("j3b_kode");
            let j3b_add: Option<String> = b_row.get("j3b_add");

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
                 WHERE d.bagan_id = ? ORDER BY d.round ASC, d.urutan ASC"
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

            bagans_payload.push(serde_json::json!({
                "kode": b_kode.unwrap_or_default(),
                "nama_bagan": b_nama,
                "nomor_tanding": nt_name.unwrap_or_default(),
                "is_bob": b_is_bob,
                "tipe_tanding": b_tipe,
                "pool": b_pool,
                "juara_1": j1_nama.unwrap_or_default(),
                "juara_1_kode": j1_kode.unwrap_or_default(),
                "juara_1_public_detail_id": j1_add.unwrap_or_default(),
                "juara_2": j2_nama.unwrap_or_default(),
                "juara_2_kode": j2_kode.unwrap_or_default(),
                "juara_2_public_detail_id": j2_add.unwrap_or_default(),
                "juara_3a": j3a_nama.unwrap_or_default(),
                "juara_3a_kode": j3a_kode.unwrap_or_default(),
                "juara_3a_public_detail_id": j3a_add.unwrap_or_default(),
                "juara_3b": j3b_nama.unwrap_or_default(),
                "juara_3b_kode": j3b_kode.unwrap_or_default(),
                "juara_3b_public_detail_id": j3b_add.unwrap_or_default(),
                "detail_bagans": dbs_payload,
            }));
        }

        let is_full_sync = bagan_ids.is_none();
        let payload = serde_json::json!({
            "event_pk": hosted_event_id,
            "bagans": bagans_payload,
            "is_full_sync": is_full_sync,
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;

        let target_url = format!("{}/api/sync/push-bagan/", sync_cfg.url.trim_end_matches('/'));
        let token = Self::get_api_token();

        let resp = client
            .post(&target_url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Gagal mengirim bagan ke server web: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("Server web mengembalikan status HTTP {}", resp.status()));
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

        // 1. Gather all finished DetailBagan
        let finished_matches = sqlx::query(
            "SELECT d.id, d.bagan_id, d.round, d.urutan, d.score1, d.score2, d.scorekecil1, d.scorekecil2, \
             d.vr1, d.vr2, d.kata1, d.kata2, d.selesai, d.pemenang \
             FROM backend_detailbagan d \
             JOIN backend_bagan b ON d.bagan_id = b.id \
             WHERE b.event_id = ? AND d.selesai = 1 \
             ORDER BY d.id ASC"
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

            // Compute next match
            let next_round = round + 1;
            let next_urutan = (urutan + 1) / 2;
            let next_db_id: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM backend_detailbagan WHERE bagan_id = ? AND round = ? AND urutan = ?"
            )
            .bind(b_id)
            .bind(next_round)
            .bind(next_urutan)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);

            let next_kr = next_db_id.map(|nid| format!("{}-{}-{}", event_code, b_id, nid));
            let target_slot = if urutan % 2 == 1 { "atlet1" } else { "atlet2" };

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
            "SELECT b.kode, b.nama_bagan, \
             j1.nama_atlet as j1_nama, j1.kode_atlet as j1_kode, \
             j2.nama_atlet as j2_nama, j2.kode_atlet as j2_kode, \
             j3a.nama_atlet as j3a_nama, j3a.kode_atlet as j3a_kode, \
             j3b.nama_atlet as j3b_nama, j3b.kode_atlet as j3b_kode \
             FROM backend_bagan b \
             LEFT JOIN backend_atlet j1 ON b.juara_1_id = j1.id \
             LEFT JOIN backend_atlet j2 ON b.juara_2_id = j2.id \
             LEFT JOIN backend_atlet j3a ON b.juara_3a_id = j3a.id \
             LEFT JOIN backend_atlet j3b ON b.juara_3b_id = j3b.id \
             WHERE b.event_id = ? AND (b.juara_1_id IS NOT NULL OR b.juara_2_id IS NOT NULL OR b.juara_3a_id IS NOT NULL OR b.juara_3b_id IS NOT NULL)"
        )
        .bind(local_event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat juara bagan: {}", e))?;

        let mut champions_payload = Vec::new();
        for cr in champion_rows {
            let b_kode: Option<String> = cr.get("kode");
            let b_nama: Option<String> = cr.get("nama_bagan");
            let j1_nama: Option<String> = cr.get("j1_nama");
            let j1_kode: Option<String> = cr.get("j1_kode");
            let j2_nama: Option<String> = cr.get("j2_nama");
            let j2_kode: Option<String> = cr.get("j2_kode");
            let j3a_nama: Option<String> = cr.get("j3a_nama");
            let j3a_kode: Option<String> = cr.get("j3a_kode");
            let j3b_nama: Option<String> = cr.get("j3b_nama");
            let j3b_kode: Option<String> = cr.get("j3b_kode");

            champions_payload.push(serde_json::json!({
                "kode": b_kode.unwrap_or_default(),
                "nama_bagan": b_nama,
                "juara_1": j1_nama.unwrap_or_default(),
                "juara_1_kode": j1_kode.unwrap_or_default(),
                "juara_2": j2_nama.unwrap_or_default(),
                "juara_2_kode": j2_kode.unwrap_or_default(),
                "juara_3a": j3a_nama.unwrap_or_default(),
                "juara_3a_kode": j3a_kode.unwrap_or_default(),
                "juara_3b": j3b_nama.unwrap_or_default(),
                "juara_3b_kode": j3b_kode.unwrap_or_default(),
            }));
        }

        let total_results = results_payload.len();
        let payload = serde_json::json!({
            "event_pk": hosted_event_id,
            "results": results_payload,
            "champions": champions_payload,
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;

        let target_url =
            format!("{}/api/sync/force-results/", sync_cfg.url.trim_end_matches('/'));
        let token = Self::get_api_token();

        let resp = client
            .post(&target_url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Gagal mengirim force sync results: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("Server web mengembalikan status HTTP {}", resp.status()));
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
            let failed = sqlx::query_scalar("SELECT COUNT(*) FROM backend_syncqueue WHERE event_id = ? AND status = 'failed'")
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
            let failed = sqlx::query_scalar("SELECT COUNT(*) FROM backend_syncqueue WHERE status = 'failed'")
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
                sqlx::query("SELECT id, event_id, endpoint, payload, status, retry_count, last_error, created_at, updated_at FROM backend_syncqueue WHERE event_id = ? AND status = 'failed' ORDER BY id ASC LIMIT ?")
                    .bind(eid).bind(limit).fetch_all(pool).await
            } else {
                sqlx::query("SELECT id, event_id, endpoint, payload, status, retry_count, last_error, created_at, updated_at FROM backend_syncqueue WHERE status = 'failed' ORDER BY id ASC LIMIT ?")
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
                ring = v.get("ring").and_then(|s| s.as_i64()).map(|i| i as i32);
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
        let token = Self::get_api_token();

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        let payload_json: serde_json::Value = serde_json::from_str(&payload_str)
            .unwrap_or_else(|_| serde_json::json!({}));

        match client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&payload_json)
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
                let err_msg = format!("HTTP error: {}", resp.status());
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
                "SELECT id FROM backend_syncqueue WHERE event_id = ? AND status = 'failed' ORDER BY id ASC"
            )
            .bind(eid)
            .fetch_all(pool)
            .await
            .unwrap_or_default()
        } else {
            sqlx::query_scalar(
                "SELECT id FROM backend_syncqueue WHERE status = 'failed' ORDER BY id ASC"
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default()
        };

        let mut success_count = 0;
        for id in items {
            if Self::retry_single_item(pool, db_path, id).await.is_ok() {
                success_count += 1;
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


