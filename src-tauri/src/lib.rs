pub mod audio;
pub mod bracket;
pub mod db;
pub mod excel_exporter;
pub mod license;
pub mod models;
pub mod referee;
pub mod server;
pub mod sync;
pub mod tatami_claim;
#[cfg(test)]
pub mod test_support;
#[cfg(test)]
mod qa_tests;

use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tauri::{Emitter, Listener, Manager};

use audio::{AudioService, SoundType};
use db::DatabaseManager;
use models::{
    AthleteMutationInput, Atlet, Bagan, BaganPodium, DrawingCategoryInfo, DrawingSummary, EnrichedAtlet,
    EnrichedDetailBagan, EnrichedPerguruan, EnrichedUtusan, Event, NomorTanding, Perguruan, SaveMatchInput, SummaryBaganResult, SummaryJuaraResult, Tatami,
    UpdateMatchAthletesInput, Utusan,
    AssignBoutAthletesInput, TeamBoutItem, TeamMatchInfo,
};
use server::{ArenaMessage, NetworkInfo};

pub struct DesktopState {
    pub db: Arc<Mutex<DatabaseManager>>,
    pub audio: AudioService,
    pub server_broadcast: broadcast::Sender<ArenaMessage>,
    pub live_tatami_cache: Arc<tokio::sync::Mutex<std::collections::HashMap<i64, serde_json::Value>>>,
}

/// Tells every LAN device (operator laptops, bracket screens) that brackets changed, so they
/// reload. Changes made over HTTP already do this in server.rs; this covers the host's own UI.
/// Referees changed (roster, photo, tatami team or a bout's panel): tablets reload them.
fn notify_referees_changed(state: &DesktopState) {
    let _ = state.server_broadcast.send(ArenaMessage {
        tatami_pk: 0,
        channel: "bracket".into(),
        payload: serde_json::json!({ "type": "referee_updated" }),
    });
}

/// A bout's approval status changed (Tatami Manager, control panel, chief referee pages)
fn notify_bout_status(state: &DesktopState, match_id: i64, status: &str, action: &str, note: Option<&str>) {
    let _ = state.server_broadcast.send(ArenaMessage {
        tatami_pk: 0,
        channel: "bracket".into(),
        payload: serde_json::json!({ "type": "bout_status", "match_id": match_id, "status": status, "action": action, "note": note }),
    });
}

fn notify_bracket_changed(state: &DesktopState) {
    let _ = state.server_broadcast.send(ArenaMessage {
        tatami_pk: 0,
        channel: "bracket".into(),
        payload: serde_json::json!({ "type": "bracket_updated" }),
    });
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct DatabaseSummary {
    pub db_path: String,
    pub media_path: String,
    pub events: Vec<Event>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct MonitorOption {
    pub id: usize,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
    pub label: String,
}

#[tauri::command]
fn get_network_info() -> NetworkInfo {
    server::get_network_info()
}

#[tauri::command]
async fn check_ip_availability() -> Result<Vec<server::IpAvailabilityInfo>, String> {
    let is_running = server::is_server_running().await;
    Ok(server::check_ip_availability(is_running).await)
}

#[tauri::command]
async fn stop_arena_server() -> Result<(), String> {
    server::stop_server().await
}

#[tauri::command]
async fn start_arena_server(
    app: tauri::AppHandle,
    selected_ip: Option<String>,
    state: tauri::State<'_, DesktopState>,
) -> Result<NetworkInfo, String> {
    if server::is_server_running().await {
        return Ok(server::get_network_info_with_ip(selected_ip.as_deref()));
    }

    let app_state = Arc::new(server::AppState {
        broadcast_tx: state.server_broadcast.clone(),
        db: state.db.clone(),
        app_handle: Some(app),
        live_tatami_cache: state.live_tatami_cache.clone(),
    });

    server::start_server(app_state).await?;
    Ok(server::get_network_info_with_ip(selected_ip.as_deref()))
}

#[tauri::command]
fn claim_tatami(tatami: i64, owner_id: String, owner_label: Option<String>, force: Option<bool>) -> Result<tatami_claim::ClaimResult, String> {
    if tatami <= 0 || owner_id.is_empty() {
        return Err("tatami dan ownerId wajib diisi".into());
    }
    Ok(tatami_claim::claim(tatami, &owner_id, owner_label.as_deref().unwrap_or(""), force.unwrap_or(false)))
}

#[tauri::command]
fn release_tatami(tatami: i64, owner_id: String) -> bool {
    tatami_claim::release(tatami, &owner_id)
}

#[tauri::command]
async fn get_server_status() -> Result<bool, String> {
    Ok(server::is_server_running().await)
}

#[tauri::command]
async fn pick_and_mount_database(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
) -> Result<Option<DatabaseSummary>, String> {
    use tauri_plugin_dialog::DialogExt;

    let file = app
        .dialog()
        .file()
        .add_filter("SQLite Tournament Database", &["sqlite3", "db", "sqlite"])
        .blocking_pick_file();

    let Some(file_path) = file else {
        return Ok(None);
    };

    let path_buf = file_path.into_path().map_err(|e| e.to_string())?;

    let mut db = state.db.lock().await;
    db.mount_database(&path_buf).await?;
    let events = db.get_events().await?;

    let db_path_str = path_buf.to_string_lossy().to_string();
    let media_path_str = db
        .get_media_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    Ok(Some(DatabaseSummary {
        db_path: db_path_str,
        media_path: media_path_str,
        events,
    }))
}


#[tauri::command]
async fn backup_database(
    state: tauri::State<'_, DesktopState>,
) -> Result<String, String> {
    let db = state.db.lock().await;
    db.backup_database(None).await
}

#[tauri::command]
async fn backup_database_to_file(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let default_name = format!(
        "ambrilindo_backup_{}.sqlite3",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );

    let file = app
        .dialog()
        .file()
        .add_filter("SQLite Database", &["sqlite3", "db", "sqlite"])
        .set_file_name(&default_name)
        .blocking_save_file();

    let Some(file_path) = file else {
        return Ok(None);
    };

    let dest = file_path.as_path().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    if dest.is_empty() {
        return Ok(None);
    }

    let db = state.db.lock().await;
    let saved = db.backup_database(Some(dest)).await?;
    Ok(Some(saved))
}

#[tauri::command]
async fn mount_database_path(
    path: String,
    state: tauri::State<'_, DesktopState>,
) -> Result<DatabaseSummary, String> {
    let mut db = state.db.lock().await;
    db.mount_database(&path).await?;
    let events = db.get_events().await?;
    let media_path_str = db
        .get_media_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    Ok(DatabaseSummary {
        db_path: path,
        media_path: media_path_str,
        events,
    })
}

#[tauri::command]
async fn auto_mount_default_database(
    state: tauri::State<'_, DesktopState>,
) -> Result<Option<DatabaseSummary>, String> {
    let candidates = [
        "tournament_data/db.sqlite3",
        "../tournament_data/db.sqlite3",
        "../../tournament_data/db.sqlite3",
        "db.sqlite3",
    ];

    for candidate in candidates {
        let p = std::path::Path::new(candidate);
        if p.exists() {
            let mut db = state.db.lock().await;
            if db.mount_database(p).await.is_ok() {
                let events = db.get_events().await.unwrap_or_default();
                let full_path = std::fs::canonicalize(p)
                    .map(|cp| cp.to_string_lossy().to_string())
                    .unwrap_or_else(|_| p.to_string_lossy().to_string());
                let media_path_str = db
                    .get_media_dir()
                    .map(|mp| mp.to_string_lossy().to_string())
                    .unwrap_or_default();

                return Ok(Some(DatabaseSummary {
                    db_path: full_path,
                    media_path: media_path_str,
                    events,
                }));
            }
        }
    }

    Ok(None)
}

#[tauri::command]
async fn get_events(
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<Event>, String> {
    let db = state.db.lock().await;
    db.get_events().await
}

#[tauri::command]
async fn create_event(
    nama_event: String,
    kode_event: Option<String>,
    state: tauri::State<'_, DesktopState>,
) -> Result<Event, String> {
    let db = state.db.lock().await;
    db.create_event(&nama_event, kode_event).await
}

#[tauri::command]
async fn get_tatamis(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<Tatami>, String> {
    let db = state.db.lock().await;
    db.get_tatamis(event_id).await
}

#[tauri::command]
async fn get_categories(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<NomorTanding>, String> {
    let db = state.db.lock().await;
    db.get_categories(event_id).await
}

#[tauri::command]
async fn get_drawing_eligible_categories(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<DrawingCategoryInfo>, String> {
    let db = state.db.lock().await;
    db.get_drawing_eligible_categories(event_id).await
}

#[tauri::command]
async fn execute_drawing_bagan(
    event_id: i64,
    category_ids: Vec<i64>,
    shuffle_type: String,
    vr_category_ids: Vec<i64>,
    state: tauri::State<'_, DesktopState>,
) -> Result<DrawingSummary, String> {
    let db = state.db.lock().await;
    let res = db.execute_drawing_bagan(event_id, category_ids, &shuffle_type, vr_category_ids).await;
    if res.is_ok() {
        notify_bracket_changed(&state);
    }
    res
}

#[tauri::command]
async fn get_drawing_eligible_festival_categories(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<DrawingCategoryInfo>, String> {
    let db = state.db.lock().await;
    db.get_drawing_eligible_festival_categories(event_id).await
}

#[tauri::command]
async fn execute_drawing_festival_bagan(
    event_id: i64,
    category_ids: Vec<i64>,
    shuffle_type: String,
    state: tauri::State<'_, DesktopState>,
) -> Result<DrawingSummary, String> {
    let db = state.db.lock().await;
    let res = db.execute_drawing_festival_bagan(event_id, category_ids, &shuffle_type).await;
    if res.is_ok() {
        notify_bracket_changed(&state);
    }
    res
}

#[tauri::command]
async fn delete_all_bagans(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let res = db.delete_all_bagans(event_id).await;
    if res.is_ok() {
        notify_bracket_changed(&state);
    }
    res
}

#[tauri::command]
async fn delete_all_festival_bagans(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<u64, String> {
    let db = state.db.lock().await;
    let res = db.delete_all_festival_bagans(event_id).await;
    if res.is_ok() {
        notify_bracket_changed(&state);
    }
    res
}

#[tauri::command]
async fn delete_category_bagans(
    event_id: i64,
    category_id: i64,
    force: Option<bool>,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let res = db.delete_category_bagans(event_id, category_id, force.unwrap_or(false)).await;
    if res.is_ok() {
        notify_bracket_changed(&state);
    }
    res
}

#[tauri::command]
async fn delete_all_athletes(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<u64, String> {
    let db = state.db.lock().await;
    db.delete_all_athletes(event_id).await
}

#[tauri::command]
async fn reset_total_tournament(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::ResetTournamentSummary, String> {
    let db = state.db.lock().await;
    db.reset_total_tournament(event_id).await
}

/// Host-only on purpose: not exposed through the LAN dispatcher.
#[tauri::command]
async fn delete_event(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::ResetTournamentSummary, String> {
    let db = state.db.lock().await;
    db.delete_event(event_id).await
}

#[tauri::command]
async fn delete_all_categories(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<u64, String> {
    let db = state.db.lock().await;
    db.delete_all_categories(event_id).await
}

#[tauri::command]
async fn delete_all_contingents(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<u64, String> {
    let db = state.db.lock().await;
    db.delete_all_contingents(event_id).await
}

#[tauri::command]
async fn delete_all_perguruans(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<u64, String> {
    let db = state.db.lock().await;
    db.delete_all_perguruans(event_id).await
}

#[tauri::command]
async fn get_bagans(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<Bagan>, String> {
    let db = state.db.lock().await;
    db.get_bagans(event_id).await
}

#[tauri::command]
async fn update_bagan_layout_type(
    bagan_id: i64,
    layout_type: String,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.update_bagan_layout_type(bagan_id, layout_type).await
}

#[tauri::command]
async fn get_detail_bagan(
    bagan_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<EnrichedDetailBagan>, String> {
    let db = state.db.lock().await;
    db.get_detail_bagan(bagan_id).await
}

#[tauri::command]
async fn get_bagan_by_id(
    bagan_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<Option<Bagan>, String> {
    let db = state.db.lock().await;
    db.get_bagan_by_id(bagan_id).await
}

#[tauri::command]
async fn get_bagan_podium(
    bagan_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<BaganPodium, String> {
    let db = state.db.lock().await;
    db.get_bagan_podium(bagan_id).await
}

#[tauri::command]
async fn get_athletes_for_bagan(
    bagan_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<EnrichedAtlet>, String> {
    let db = state.db.lock().await;
    db.get_athletes_for_bagan(bagan_id).await
}

#[tauri::command]
async fn update_bagan_details(
    app: tauri::AppHandle,
    bagan_id: i64,
    nama_bagan: Option<String>,
    matchups: Vec<UpdateMatchAthletesInput>,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.update_bagan_details(bagan_id, nama_bagan, matchups).await?;
    let _ = app.emit("bracket_updated", ());
    notify_bracket_changed(&state);
    Ok(())
}

#[tauri::command]
async fn get_summary_juara(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<SummaryJuaraResult, String> {
    let db = state.db.lock().await;
    db.get_summary_juara(event_id).await
}

#[tauri::command]
async fn get_summary_bagan(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<SummaryBaganResult, String> {
    let db = state.db.lock().await;
    db.get_summary_bagan(event_id).await
}

#[tauri::command]
async fn close_control_panel_window(
    app: tauri::AppHandle,
    match_id: i64,
) -> Result<(), String> {
    use tauri::Manager;
    let win_label = format!("control-panel-{}", match_id);
    if let Some(win) = app.get_webview_window(&win_label) {
        let _ = win.close();
    }
    // Defensively close any remaining control-panel windows
    for (label, win) in app.webview_windows() {
        if label.starts_with("control-panel") {
            let _ = win.close();
        }
    }
    Ok(())
}

#[tauri::command]
async fn save_match_result(
    app: tauri::AppHandle,
    input: SaveMatchInput,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let (finished_id, finished) = (input.match_id, input.selesai);
    let (pool_opt, db_path_opt) = {
        db.save_match_result(input).await?;
        // history: who officiated a finished bout (its tatami team, unless set per bout)
        if finished {
            db.snapshot_bout_panel(finished_id).await;
        }
        (db.get_pool_cloned(), db.get_active_db_path_cloned())
    };
    notify_bracket_changed(&state);

    if let Some(pool) = pool_opt {
        let app_handle = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Ok((synced, failed)) = sync::SyncService::process_sync_queue(&pool, db_path_opt.as_ref()).await {
                if synced > 0 || failed > 0 {
                    let _ = app_handle.emit("sync_queue_changed", serde_json::json!({
                        "synced": synced,
                        "failed": failed
                    }));
                }
            }
        });
    }

    Ok(())
}

#[tauri::command]
async fn open_control_panel_window(
    state: tauri::State<'_, DesktopState>,
    app: tauri::AppHandle,
    match_id: i64,
    bagan_id: i64,
    tatami: Option<i64>,
    host_url: Option<String>,
) -> Result<(), String> {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

    // License: Kata / Kumite module, decided by the bracket's discipline.
    if let Some(host) = host_url.as_deref().map(str::trim).filter(|h| !h.is_empty()) {
        // Operator laptop: the host's license applies (its DB holds the bracket).
        let st = host_license_status(host).await?;
        license::check(&st, license::Module::Kata, license::now_secs())
            .or_else(|_| license::check(&st, license::Module::Kumite, license::now_secs()))?;
    } else {
        let discipline = if bagan_id > 0 {
            let row = match state.db.lock().await.get_pool() {
                Ok(pool) => sqlx::query_as::<_, (Option<String>, Option<String>)>(
                    "SELECT CAST(tipe_tanding AS TEXT), nama_bagan FROM backend_bagan WHERE id = ?",
                )
                .bind(bagan_id)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten(),
                Err(_) => None,
            };
            row.and_then(|(tt, name)| license::discipline_of(tt.as_deref(), name.as_deref()))
        } else {
            None
        };
        license::ensure_discipline(discipline)?;
    }

    let win_label = format!("control-panel-{}", match_id);
    let mut t_num = tatami.unwrap_or(1);

    // Defensive normalization: if tatami is passed as DB PK (>= 20), resolve to tatami_number
    if t_num >= 20 {
        if let Ok(pool) = state.db.lock().await.get_pool() {
            if let Ok(Some(resolved)) = sqlx::query_scalar::<_, i32>("SELECT tatami_number FROM backend_tatami WHERE id = ?")
                .bind(t_num)
                .fetch_optional(pool)
                .await
            {
                t_num = resolved as i64;
            }
        }
    }

    // Set when this laptop is an operator tambahan: the window must talk to the host,
    // and the host (not this laptop's DB) records the running match.
    let host_param = host_query_param(host_url.as_deref());

    // Notify match is running on this tatami and enqueue to sync
    let t_num_i32 = t_num as i32;
    if match_id > 0 && host_param.is_empty() {
        let (pool_opt, db_path_opt) = {
            let db = state.db.lock().await;
            let _ = db.notify_match_running(match_id, t_num_i32).await;
            (db.get_pool_cloned(), db.get_active_db_path_cloned())
        };
        if let Some(pool) = pool_opt {
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Ok((synced, failed)) = sync::SyncService::process_sync_queue(&pool, db_path_opt.as_ref()).await {
                    if synced > 0 || failed > 0 {
                        let _ = app_handle.emit("sync_queue_changed", serde_json::json!({
                            "synced": synced,
                            "failed": failed
                        }));
                    }
                }
            });
        }

        // Immediately build live payload, store into cache, and broadcast to arena server
        if let Ok(Some(current_m)) = state.db.lock().await.get_tatami_current_match(t_num_i32, None).await {
            let live_payload = server::build_live_state_from_match(t_num, &current_m);
            {
                let mut cache = state.live_tatami_cache.lock().await;
                cache.insert(t_num, live_payload.clone());
            }
            let _ = state.server_broadcast.send(server::ArenaMessage {
                tatami_pk: t_num,
                channel: "bracket".to_string(),
                payload: serde_json::json!({
                    "type": "tatami_live_state",
                    "payload": live_payload
                }),
            });
            let _ = app.emit("tatami_live_state", &live_payload);
        }
    }

    // If window for the exact same match already exists:
    // Close any other control panels, bring this one forward, reload, and focus
    if let Some(win) = app.get_webview_window(&win_label) {
        for (label, other_win) in app.webview_windows() {
            if label.starts_with("control-panel") && label != win_label {
                let _ = other_win.close();
            }
        }
        let _ = win.show();
        let _ = win.maximize();
        let _ = win.set_focus();
        let _ = win.eval("window.location.reload()");
        return Ok(());
    }

    // Close any previously opened control panel windows so only the latest one stays active
    for (label, existing_win) in app.webview_windows() {
        if label.starts_with("control-panel") {
            let _ = existing_win.close();
        }
    }

    let (url, title) = if match_id == 0 {
        (
            format!("/control-panel?matchId=0&baganId=0&isFestival=true&tatami={}{}", t_num, host_param),
            format!("Ambrilindo - Control Panel Festival (Bagan 000) - Tatami {}", t_num),
        )
    } else {
        (
            format!("/control-panel?matchId={}&baganId={}&tatami={}{}", match_id, bagan_id, t_num, host_param),
            format!("Ambrilindo - Control Panel Partai #{}", match_id),
        )
    };

    let _win = WebviewWindowBuilder::new(&app, &win_label, WebviewUrl::App(url.into()))
        .title(title)
        .inner_size(1366.0, 768.0)
        // Smaller operator laptops (1280×720 and up) still fit; opens full window
        .min_inner_size(960.0, 600.0)
        .center()
        .maximized(true)
        .build()
        .map_err(|e| format!("Gagal membuka window control panel: {}", e))?;

    Ok(())
}

/// License status of the host laptop (operator laptops follow the host's license).
async fn host_license_status(host: &str) -> Result<license::LicenseStatus, String> {
    let url = format!("{}/api/command/get_license_status", host.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;
    let res = client
        .post(&url)
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|_| "Tidak bisa mengecek lisensi laptop host. Pastikan terhubung ke host.".to_string())?;
    let body: serde_json::Value = res.json().await.map_err(|_| "Balasan lisensi dari host tidak dikenali.".to_string())?;
    serde_json::from_value(body.get("data").cloned().unwrap_or_default())
        .map_err(|_| "Host belum mendukung lisensi. Perbarui aplikasi di laptop host.".to_string())
}

// ---------------------------------------------------------------------------
// License commands (run on the host/solo computer only)
// ---------------------------------------------------------------------------

#[tauri::command]
async fn get_license_status() -> Result<license::LicenseStatus, String> {
    Ok(license::current())
}

#[tauri::command]
async fn activate_license(key: String, app: tauri::AppHandle) -> Result<license::LicenseStatus, String> {
    let st = license::activate(&key).await?;
    let _ = app.emit("license_changed", &st);
    Ok(st)
}

#[tauri::command]
async fn refresh_license(app: tauri::AppHandle) -> Result<license::LicenseStatus, String> {
    let res = license::refresh().await;
    let st = license::current();
    let _ = app.emit("license_changed", &st);
    match res {
        Ok(_) => Ok(st),
        Err(e) => Err(e),
    }
}

#[tauri::command]
async fn deactivate_license(app: tauri::AppHandle) -> Result<license::LicenseStatus, String> {
    let st = license::deactivate().await?;
    let _ = app.emit("license_changed", &st);
    Ok(st)
}

/// `&hostUrl=<encoded>` when a host URL is given, otherwise empty.
fn host_query_param(host_url: Option<&str>) -> String {
    match host_url.map(str::trim) {
        Some(h) if !h.is_empty() => format!("&hostUrl={}", helper_url_encode(h)),
        _ => String::new(),
    }
}

fn helper_url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            out.push(b as char);
        } else if b == b' ' {
            out.push_str("%20");
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

#[tauri::command]
async fn open_festival_control_panel(
    app: tauri::AppHandle,
    tatami: Option<i64>,
    aka_nama: Option<String>,
    aka_utusan: Option<String>,
    ao_nama: Option<String>,
    ao_utusan: Option<String>,
    cat_name: Option<String>,
    discipline: Option<String>,
    host_url: Option<String>,
) -> Result<(), String> {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

    // License: festival panels need the module of their discipline.
    let fest_discipline = license::discipline_of(discipline.as_deref(), cat_name.as_deref());
    if let Some(host) = host_url.as_deref().map(str::trim).filter(|h| !h.is_empty()) {
        let st = host_license_status(host).await?;
        let now = license::now_secs();
        match fest_discipline {
            Some(m) => license::check(&st, m, now)?,
            None => license::check(&st, license::Module::Kata, now)
                .or_else(|_| license::check(&st, license::Module::Kumite, now))?,
        }
    } else {
        license::ensure_discipline(fest_discipline)?;
    }

    let win_label = "control-panel-0".to_string();
    let fest_tatami_num = tatami.unwrap_or(1);
    let t_num = tatami.unwrap_or(1);

    for (label, existing_win) in app.webview_windows() {
        if label.starts_with("control-panel") {
            let _ = existing_win.close();
        }
    }

    let mut query = format!("matchId=0&baganId=0&isFestival=true&tatami={}", fest_tatami_num);
    if let Some(ref n) = aka_nama {
        query.push_str(&format!("&aka={}", helper_url_encode(n)));
    }
    if let Some(ref u) = aka_utusan {
        query.push_str(&format!("&aka_club={}", helper_url_encode(u)));
    }
    if let Some(ref n) = ao_nama {
        query.push_str(&format!("&ao={}", helper_url_encode(n)));
    }
    if let Some(ref u) = ao_utusan {
        query.push_str(&format!("&ao_club={}", helper_url_encode(u)));
    }
    if let Some(ref c) = cat_name {
        query.push_str(&format!("&cat={}", helper_url_encode(c)));
    }
    if let Some(ref d) = discipline {
        query.push_str(&format!("&disc={}", helper_url_encode(d)));
    }

    query.push_str(&host_query_param(host_url.as_deref()));
    let url = format!("/control-panel?{}", query);
    let title = format!("Ambrilindo - Control Panel Festival (Bagan 000) - Tatami {}", t_num);

    let _win = WebviewWindowBuilder::new(&app, &win_label, WebviewUrl::App(url.into()))
        .title(title)
        .inner_size(1366.0, 768.0)
        // Smaller operator laptops (1280×720 and up) still fit; opens full window
        .min_inner_size(960.0, 600.0)
        .center()
        .maximized(true)
        .build()
        .map_err(|e| format!("Gagal membuka window control panel festival: {}", e))?;

    Ok(())
}

#[tauri::command]
async fn get_media_data_url(
    relative_path: String,
    state: tauri::State<'_, DesktopState>,
) -> Result<Option<String>, String> {
    let db = state.db.lock().await;
    if let Some(media_dir) = db.get_media_dir() {
        let clean_path = relative_path.trim_start_matches('/').trim_start_matches('\\');
        let full_path = media_dir.join(clean_path);
        if full_path.exists() && full_path.is_file() {
            if let Ok(bytes) = std::fs::read(&full_path) {
                let ext = full_path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("png")
                    .to_lowercase();
                let mime = match ext.as_str() {
                    "png" => "image/png",
                    "webp" => "image/webp",
                    "jpg" | "jpeg" => "image/jpeg",
                    "svg" => "image/svg+xml",
                    _ => "application/octet-stream",
                };
                use base64::Engine;
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                return Ok(Some(format!("data:{};base64,{}", mime, b64)));
            }
        }
    }
    Ok(None)
}

#[tauri::command]
async fn get_athletes(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<Atlet>, String> {
    let db = state.db.lock().await;
    db.get_athletes(event_id).await
}

#[tauri::command]
async fn get_enriched_athletes(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<EnrichedAtlet>, String> {
    let db = state.db.lock().await;
    db.get_enriched_athletes(event_id).await
}

#[tauri::command]
async fn create_athlete(
    input: AthleteMutationInput,
    state: tauri::State<'_, DesktopState>,
) -> Result<EnrichedAtlet, String> {
    let db = state.db.lock().await;
    db.create_athlete(input).await
}

#[tauri::command]
async fn update_athlete(
    id: i64,
    input: AthleteMutationInput,
    state: tauri::State<'_, DesktopState>,
) -> Result<EnrichedAtlet, String> {
    let db = state.db.lock().await;
    db.update_athlete(id, input).await
}

#[tauri::command]
async fn delete_athlete(
    id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.delete_athlete(id).await
}

#[tauri::command]
async fn delete_athletes_batch(
    ids: Vec<i64>,
    state: tauri::State<'_, DesktopState>,
) -> Result<u64, String> {
    let db = state.db.lock().await;
    db.delete_athletes_batch(ids).await
}

#[tauri::command]
async fn toggle_athlete_priority(
    id: i64,
    is_priority: bool,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.toggle_athlete_priority(id, is_priority).await
}

#[tauri::command]
async fn get_perguruans(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<Perguruan>, String> {
    let db = state.db.lock().await;
    db.get_perguruans(event_id).await
}

#[tauri::command]
async fn get_contingents(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<Utusan>, String> {
    let db = state.db.lock().await;
    db.get_contingents(event_id).await
}

#[tauri::command]
async fn get_enriched_contingents(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<EnrichedUtusan>, String> {
    let db = state.db.lock().await;
    db.get_enriched_contingents(event_id).await
}

#[tauri::command]
async fn create_contingent(
    event_id: i64,
    nama_utusan: String,
    logo: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
) -> Result<EnrichedUtusan, String> {
    let db = state.db.lock().await;
    let res = db.create_contingent(event_id, nama_utusan, logo).await?;
    let _ = state.server_broadcast.send(server::ArenaMessage {
        tatami_pk: 0,
        channel: "master-data".into(),
        payload: serde_json::json!({ "type": "contingent_updated", "event_id": event_id }),
    });
    let _ = app.emit("master_data_updated", serde_json::json!({ "type": "contingent_updated", "event_id": event_id }));
    Ok(res)
}

#[tauri::command]
async fn update_contingent(
    id: i64,
    nama_utusan: String,
    logo: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
) -> Result<EnrichedUtusan, String> {
    let db = state.db.lock().await;
    let res = db.update_contingent(id, nama_utusan, logo).await?;
    let _ = state.server_broadcast.send(server::ArenaMessage {
        tatami_pk: 0,
        channel: "master-data".into(),
        payload: serde_json::json!({ "type": "contingent_updated" }),
    });
    let _ = app.emit("master_data_updated", serde_json::json!({ "type": "contingent_updated" }));
    Ok(res)
}

#[tauri::command]
async fn delete_contingent(
    id: i64,
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.delete_contingent(id).await?;
    let _ = state.server_broadcast.send(server::ArenaMessage {
        tatami_pk: 0,
        channel: "master-data".into(),
        payload: serde_json::json!({ "type": "contingent_updated" }),
    });
    let _ = app.emit("master_data_updated", serde_json::json!({ "type": "contingent_updated" }));
    Ok(())
}

#[tauri::command]
async fn get_enriched_perguruans(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<EnrichedPerguruan>, String> {
    let db = state.db.lock().await;
    db.get_enriched_perguruans(event_id).await
}

#[tauri::command]
async fn create_perguruan(
    event_id: i64,
    nama_perguruan: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
) -> Result<EnrichedPerguruan, String> {
    let db = state.db.lock().await;
    let res = db.create_perguruan(event_id, nama_perguruan).await?;
    let _ = state.server_broadcast.send(server::ArenaMessage {
        tatami_pk: 0,
        channel: "master-data".into(),
        payload: serde_json::json!({ "type": "perguruan_updated", "event_id": event_id }),
    });
    let _ = app.emit("master_data_updated", serde_json::json!({ "type": "perguruan_updated", "event_id": event_id }));
    Ok(res)
}

#[tauri::command]
async fn update_perguruan(
    id: i64,
    nama_perguruan: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
) -> Result<EnrichedPerguruan, String> {
    let db = state.db.lock().await;
    let res = db.update_perguruan(id, nama_perguruan).await?;
    let _ = state.server_broadcast.send(server::ArenaMessage {
        tatami_pk: 0,
        channel: "master-data".into(),
        payload: serde_json::json!({ "type": "perguruan_updated" }),
    });
    let _ = app.emit("master_data_updated", serde_json::json!({ "type": "perguruan_updated" }));
    Ok(res)
}

#[tauri::command]
async fn delete_perguruan(
    id: i64,
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.delete_perguruan(id).await?;
    let _ = state.server_broadcast.send(server::ArenaMessage {
        tatami_pk: 0,
        channel: "master-data".into(),
        payload: serde_json::json!({ "type": "perguruan_updated" }),
    });
    let _ = app.emit("master_data_updated", serde_json::json!({ "type": "perguruan_updated" }));
    Ok(())
}

#[tauri::command]
async fn save_contingent_logo(
    file_name: String,
    base64_data: String,
    state: tauri::State<'_, DesktopState>,
) -> Result<String, String> {
    let db = state.db.lock().await;
    db.save_contingent_logo(&file_name, &base64_data).await
}

#[tauri::command]
async fn get_match_juri_scores(
    match_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<serde_json::Value, String> {
    let db = state.db.lock().await;
    let (s1, s2) = db.get_match_juri_scores(match_id).await?;
    Ok(serde_json::json!({ "juri_scores1": s1, "juri_scores2": s2 }))
}

#[tauri::command]
async fn get_team_match_info(
    match_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<TeamMatchInfo, String> {
    let db = state.db.lock().await;
    db.get_team_match_info(match_id).await
}

#[tauri::command]
async fn assign_bout_athletes(
    input: AssignBoutAthletesInput,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.assign_bout_athletes(input).await
}

#[tauri::command]
async fn add_team_bout(
    parent_match_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<TeamBoutItem, String> {
    let db = state.db.lock().await;
    db.add_team_bout(parent_match_id).await
}

#[tauri::command]
async fn delete_team_bout(
    bout_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.delete_team_bout(bout_id).await
}

#[tauri::command]
async fn finish_team_match(
    parent_match_id: i64,
    winner_override: Option<String>,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let res = db.finish_team_match(parent_match_id, winner_override).await;
    if res.is_ok() {
        notify_bracket_changed(&state);
    }
    res
}

#[tauri::command]
async fn create_tatami(
    event_id: i64,
    tatami_number: i32,
    state: tauri::State<'_, DesktopState>,
) -> Result<Tatami, String> {
    let db = state.db.lock().await;
    db.create_tatami(event_id, tatami_number).await
}

#[tauri::command]
async fn delete_tatami(
    tatami_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.delete_tatami(tatami_id).await
}

#[tauri::command]
async fn update_tatami_number(
    tatami_id: i64,
    new_number: i32,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.update_tatami_number(tatami_id, new_number).await
}

#[tauri::command]
fn play_buzzer(
    sound: String,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let sound_type = match sound.to_lowercase().as_str() {
        "warning" => SoundType::Warning,
        "finish" => SoundType::Finish,
        other => return Err(format!("Tipe suara tidak dikenal: {}", other)),
    };
    state.audio.play(sound_type)
}

#[tauri::command]
fn get_available_monitors(app: tauri::AppHandle) -> Result<Vec<MonitorOption>, String> {
    let primary_opt = app.primary_monitor().ok().flatten();
    let primary_pos = primary_opt.as_ref().map(|p| (p.position().x, p.position().y));

    let monitors = app.available_monitors().map_err(|e| e.to_string())?;
    let mut list = Vec::new();

    for (idx, m) in monitors.into_iter().enumerate() {
        let name = m.name().map(|s| s.to_string()).unwrap_or_else(|| format!("Display {}", idx + 1));
        let size = m.size();
        let pos = m.position();
        let is_primary = match primary_pos {
            Some(pp) => pp.0 == pos.x && pp.1 == pos.y,
            None => idx == 0,
        };
        let label = format!(
            "Monitor {} ({}) - {}x{}{}",
            idx + 1,
            name,
            size.width,
            size.height,
            if is_primary { " [Utama]" } else { "" }
        );
        list.push(MonitorOption {
            id: idx,
            name,
            width: size.width,
            height: size.height,
            is_primary,
            label,
        });
    }

    Ok(list)
}

#[tauri::command]
async fn project_scoreboard(
    app: tauri::AppHandle,
    monitor_id: usize,
    tatami_number: u32,
    host_url: Option<String>,
) -> Result<(), String> {
    use tauri::{Emitter, WebviewUrl, WebviewWindowBuilder};

    let monitors = app.available_monitors().map_err(|e| e.to_string())?;
    let monitor = monitors.get(monitor_id).ok_or_else(|| "Monitor tidak ditemukan".to_string())?;

    let win_label = format!("scoreboard-tatami-{}", tatami_number);

    // If window already exists, bring it forward and update position
    if let Some(win) = app.get_webview_window(&win_label) {
        let _ = win.set_position(tauri::Position::Physical(*monitor.position()));
        let _ = win.set_fullscreen(true);
        let _ = win.set_focus();
        let _ = app.emit("scoreboard_status_changed", ());
        return Ok(());
    }

    let url = format!("/scoreboard?tatami={}{}", tatami_number, host_query_param(host_url.as_deref()));
    let _win = WebviewWindowBuilder::new(&app, &win_label, WebviewUrl::App(url.into()))
        .title(format!("Scoreboard Arena Tatami {}", tatami_number))
        .position(monitor.position().x as f64, monitor.position().y as f64)
        .inner_size(monitor.size().width as f64, monitor.size().height as f64)
        .fullscreen(true)
        .build()
        .map_err(|e| format!("Gagal membuka jendela scoreboard: {}", e))?;

    let _ = app.emit("scoreboard_status_changed", ());
    Ok(())
}

#[tauri::command]
async fn close_scoreboard(
    app: tauri::AppHandle,
    tatami_number: u32,
) -> Result<(), String> {
    use tauri::{Emitter, Manager};
    let win_label = format!("scoreboard-tatami-{}", tatami_number);
    if let Some(win) = app.get_webview_window(&win_label) {
        let _ = win.close();
    }
    let _ = app.emit("scoreboard_status_changed", ());
    Ok(())
}

#[tauri::command]
async fn get_running_scoreboards(
    app: tauri::AppHandle,
) -> Result<Vec<u32>, String> {
    use tauri::Manager;
    let mut running = Vec::new();
    for i in 1..=10 {
        let win_label = format!("scoreboard-tatami-{}", i);
        if let Some(win) = app.get_webview_window(&win_label) {
            if win.is_visible().unwrap_or(false) {
                running.push(i);
            }
        }
    }
    Ok(running)
}

#[tauri::command]
async fn get_sync_config(
    state: tauri::State<'_, DesktopState>,
) -> Result<sync::SyncConfig, String> {
    let db = state.db.lock().await;
    Ok(sync::SyncService::get_sync_config(db.get_db_path()))
}

#[tauri::command]
async fn set_sync_config(
    mode: String,
    custom_url: Option<String>,
    state: tauri::State<'_, DesktopState>,
) -> Result<sync::SyncConfig, String> {
    let db = state.db.lock().await;
    sync::SyncService::set_sync_config(db.get_db_path(), &mode, custom_url)
}

#[tauri::command]
fn get_sync_auth_status() -> sync::SyncAuthStatus {
    sync::SyncService::auth_status()
}

#[tauri::command]
async fn test_sync_connection(url: String) -> Result<sync::PingResult, String> {
    Ok(sync::SyncService::ping_server(&url).await)
}

#[tauri::command]
async fn get_hosted_events(url: String) -> Result<Vec<sync::HostedEventOption>, String> {
    sync::SyncService::fetch_hosted_events(&url).await
}

#[tauri::command]
async fn get_event_mapping(
    local_event_id: i64,
    local_event_name: Option<String>,
    target_mode: String,
    state: tauri::State<'_, DesktopState>,
) -> Result<sync::EventMapping, String> {
    let db = state.db.lock().await;
    Ok(sync::SyncService::get_event_mapping(
        db.get_db_path(),
        local_event_id,
        local_event_name.as_deref(),
        &target_mode,
    ))
}

#[tauri::command]
async fn set_event_mapping(
    local_event_id: i64,
    hosted_event_id: i64,
    hosted_event_name: String,
    target_mode: String,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    sync::SyncService::set_event_mapping(
        db.get_db_path(),
        local_event_id,
        hosted_event_id,
        &hosted_event_name,
        &target_mode,
    )
}

#[tauri::command]
async fn pull_athletes_from_hosted(
    local_event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<sync::PullResult, String> {
    let db = state.db.lock().await;
    let pool = db.get_pool()?;
    sync::SyncService::pull_athletes_from_hosted(pool, db.get_db_path(), local_event_id).await
}

#[tauri::command]
async fn push_bagan_to_hosted(
    local_event_id: i64,
    bagan_ids: Option<Vec<i64>>,
    state: tauri::State<'_, DesktopState>,
) -> Result<String, String> {
    let db = state.db.lock().await;
    let pool = db.get_pool()?;
    sync::SyncService::push_bagan_to_hosted(pool, db.get_db_path(), local_event_id, bagan_ids).await
}

#[tauri::command]
async fn force_sync_results_to_hosted(
    local_event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<String, String> {
    let db = state.db.lock().await;
    let pool = db.get_pool()?;
    sync::SyncService::force_sync_results_to_hosted(pool, db.get_db_path(), local_event_id).await
}

#[tauri::command]
async fn toggle_live_sync(
    local_event_id: i64,
    enabled: bool,
    state: tauri::State<'_, DesktopState>,
) -> Result<bool, String> {
    let db = state.db.lock().await;
    let pool = db.get_pool()?;
    sync::SyncService::toggle_live_sync(pool, local_event_id, enabled).await
}

#[tauri::command]
async fn get_sync_queue_stats(
    event_id: Option<i64>,
    state: tauri::State<'_, DesktopState>,
) -> Result<sync::SyncQueueStats, String> {
    let db = state.db.lock().await;
    let pool = db.get_pool()?;
    sync::SyncService::get_queue_stats(pool, event_id).await
}

#[tauri::command]
async fn get_sync_queue_items(
    event_id: Option<i64>,
    status_filter: Option<String>,
    limit: Option<i64>,
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<sync::SyncQueueEntry>, String> {
    let db = state.db.lock().await;
    let pool = db.get_pool()?;
    sync::SyncService::get_queue_items(pool, event_id, status_filter, limit.unwrap_or(200)).await
}

#[tauri::command]
async fn retry_sync_item(
    item_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<bool, String> {
    let db = state.db.lock().await;
    let pool = db.get_pool()?;
    sync::SyncService::retry_single_item(pool, db.get_db_path(), item_id).await
}

#[tauri::command]
async fn retry_all_failed_sync(
    event_id: Option<i64>,
    state: tauri::State<'_, DesktopState>,
) -> Result<usize, String> {
    let db = state.db.lock().await;
    let pool = db.get_pool()?;
    sync::SyncService::retry_all_failed(pool, db.get_db_path(), event_id).await
}

#[tauri::command]
async fn mark_sync_item_synced(
    item_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let pool = db.get_pool()?;
    sync::SyncService::mark_item_synced(pool, item_id).await
}

#[tauri::command]
async fn delete_sync_item(
    item_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let pool = db.get_pool()?;
    sync::SyncService::delete_queue_item(pool, item_id).await
}

#[tauri::command]
async fn clear_synced_items(
    event_id: Option<i64>,
    state: tauri::State<'_, DesktopState>,
) -> Result<u64, String> {
    let db = state.db.lock().await;
    let pool = db.get_pool()?;
    sync::SyncService::clear_synced_items(pool, event_id).await
}

#[tauri::command]
async fn clear_all_sync_queue(
    event_id: Option<i64>,
    state: tauri::State<'_, DesktopState>,
) -> Result<u64, String> {
    let db = state.db.lock().await;
    let pool = db.get_pool()?;
    sync::SyncService::clear_all_queue(pool, event_id).await
}

#[tauri::command]
async fn get_festival_roster_data(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::FestivalRosterData, String> {
    let db = state.db.lock().await;
    db.get_festival_roster_data(event_id).await
}

#[tauri::command]
async fn get_festival_booklet_data(
    event_id: i64,
    tatami_id: Option<i64>,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::FestivalBookletData, String> {
    let db = state.db.lock().await;
    db.get_festival_booklet_data(event_id, tatami_id).await
}

#[tauri::command]
async fn get_festival_format(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::FestivalFormatInfo, String> {
    let db = state.db.lock().await;
    db.get_festival_format(event_id).await
}

#[tauri::command]
async fn set_festival_format(
    event_id: i64,
    format: String,
    delete_bagans: bool,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::FestivalFormatInfo, String> {
    let db = state.db.lock().await;
    db.set_festival_format(event_id, format, delete_bagans).await
}

#[tauri::command]
async fn save_festival_roster_state(
    event_id: i64,
    roster_state: models::FestivalRosterState,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.save_festival_roster_state(event_id, roster_state).await
}

#[tauri::command]
async fn auto_pair_festival_category(
    event_id: i64,
    nomor_tanding_id: Option<i64>,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::FestivalRosterData, String> {
    let db = state.db.lock().await;
    db.auto_pair_festival_category(event_id, nomor_tanding_id).await
}

#[tauri::command]
async fn get_festival_mix_groups(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<models::FestivalMixGroup>, String> {
    let db = state.db.lock().await;
    db.get_festival_mix_groups(event_id).await
}

#[tauri::command]
async fn save_festival_mix_groups(
    event_id: i64,
    mix_groups: Vec<models::FestivalMixGroup>,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.save_festival_mix_groups(event_id, mix_groups).await
}

#[tauri::command]
async fn mix_festival_categories(
    event_id: i64,
    source_ids: Vec<i64>,
    target_id: Option<i64>,
    new_name: String,
    delete_sources: bool,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::FestivalRosterData, String> {
    let db = state.db.lock().await;
    db.mix_festival_categories(event_id, source_ids, target_id, new_name, delete_sources).await
}

#[tauri::command]
async fn get_enriched_categories(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<models::EnrichedNomorTanding>, String> {
    let db = state.db.lock().await;
    db.get_enriched_categories(event_id).await
}

#[tauri::command]
async fn create_category(
    input: models::CategoryMutationInput,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::EnrichedNomorTanding, String> {
    let db = state.db.lock().await;
    db.create_category(input).await
}

#[tauri::command]
async fn update_category(
    input: models::CategoryMutationInput,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::EnrichedNomorTanding, String> {
    let db = state.db.lock().await;
    db.update_category(input).await
}

#[tauri::command]
async fn toggle_category_vr(
    category_id: i64,
    has_vr: bool,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.toggle_category_vr(category_id, has_vr).await
}

#[tauri::command]
async fn delete_category(
    category_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.delete_category(category_id).await
}

#[tauri::command]
async fn get_category_athletes(
    category_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<models::EnrichedAtlet>, String> {
    let db = state.db.lock().await;
    db.get_category_athletes(category_id).await
}

#[tauri::command]
async fn get_timetable_editor_data(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::TimetableEditorData, String> {
    let db = state.db.lock().await;
    db.get_timetable_editor_data(event_id).await
}

#[tauri::command]
async fn save_timetable_data(
    event_id: i64,
    payload: models::SaveTimetablePayload,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.save_timetable_data(event_id, payload).await
}

#[tauri::command]
async fn add_timetable_day(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::TimetableDay, String> {
    let db = state.db.lock().await;
    db.add_timetable_day(event_id).await
}

#[tauri::command]
async fn delete_timetable_day(
    day_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.delete_timetable_day(day_id).await
}

#[tauri::command]
async fn reset_timetable(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.reset_timetable_data(event_id).await
}

#[tauri::command]
async fn get_kop_surat(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<Option<models::KopSurat>, String> {
    let db = state.db.lock().await;
    db.get_kop_surat(event_id).await
}

#[tauri::command]
async fn save_kop_surat(
    event_id: i64,
    nama_organisasi: String,
    alamat: String,
    kontak: String,
    logo: Option<String>,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::KopSurat, String> {
    let db = state.db.lock().await;
    db.save_kop_surat(event_id, nama_organisasi, alamat, kontak, logo).await
}

#[tauri::command]
async fn get_tatami_booklet_data(
    event_id: i64,
    day_id: i64,
    tatami_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::TatamiBookletData, String> {
    let db = state.db.lock().await;
    db.get_tatami_booklet_data(event_id, day_id, tatami_id).await
}

#[tauri::command]
async fn get_wasits(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<models::EnrichedWasit>, String> {
    let db = state.db.lock().await;
    db.get_wasits(event_id).await
}

#[tauri::command]
async fn save_wasit(
    input: models::SaveWasitInput,
    state: tauri::State<'_, DesktopState>,
) -> Result<i64, String> {
    let db = state.db.lock().await;
    let res = db.save_wasit(input).await;
    if res.is_ok() {
        notify_referees_changed(&state);
    }
    res
}

#[tauri::command]
async fn delete_wasit(
    wasit_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let res = db.delete_wasit(wasit_id).await;
    if res.is_ok() {
        notify_referees_changed(&state);
    }
    res
}

#[tauri::command]
async fn get_tatami_panel_overview(
    event_id: i64,
    tatami_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::TatamiPanelOverview, String> {
    let db = state.db.lock().await;
    db.get_tatami_panel_overview(event_id, tatami_id).await
}

#[tauri::command]
async fn assign_wasit_tatami(
    input: models::AssignWasitTatamiInput,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let res = db.assign_wasit_tatami(input).await.map(|_| ());
    if res.is_ok() {
        notify_referees_changed(&state);
    }
    res
}

#[tauri::command]
async fn remove_wasit_tatami(
    assignment_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let res = db.remove_wasit_tatami(assignment_id).await;
    if res.is_ok() {
        notify_referees_changed(&state);
    }
    res
}

#[tauri::command]
async fn clear_tatami_panel(
    tatami_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let res = db.clear_tatami_panel(tatami_id).await;
    if res.is_ok() {
        notify_referees_changed(&state);
    }
    res
}

#[tauri::command]
async fn auto_assign_tatami_panel(
    input: models::AutoAssignPanelInput,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let res = db.auto_assign_tatami_panel(input).await;
    if res.is_ok() {
        notify_referees_changed(&state);
    }
    res
}

#[tauri::command]
async fn preview_wasit_import(
    event_id: i64,
    file_name: String,
    base64_data: String,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::WasitImportPreview, String> {
    let db = state.db.lock().await;
    db.preview_wasit_import(event_id, &file_name, &base64_data).await
}

#[tauri::command]
async fn import_wasits(
    event_id: i64,
    rows: Vec<models::WasitImportRow>,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::WasitImportSummary, String> {
    let db = state.db.lock().await;
    let res = db.import_wasits(event_id, rows).await;
    if res.is_ok() {
        notify_referees_changed(&state);
    }
    res
}

#[tauri::command]
async fn save_wasit_photo(
    wasit_id: i64,
    base64_data: String,
    state: tauri::State<'_, DesktopState>,
) -> Result<String, String> {
    let db = state.db.lock().await;
    let res = db.save_wasit_photo(wasit_id, &base64_data).await;
    if res.is_ok() {
        notify_referees_changed(&state);
    }
    res
}

#[tauri::command]
async fn get_bout_panel(
    match_id: i64,
    tatami_number: i32,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::BoutPanel, String> {
    let db = state.db.lock().await;
    db.get_bout_panel(match_id, tatami_number).await
}

#[tauri::command]
async fn save_bout_panel(
    match_id: i64,
    seats: Vec<models::BoutSeatInput>,
    judges: Option<i64>,
    tatami_number: Option<i32>,
    expected_rev: Option<i64>,
    state: tauri::State<'_, DesktopState>,
) -> Result<i64, String> {
    let db = state.db.lock().await;
    let res = db.save_bout_panel(match_id, seats, judges, tatami_number, expected_rev).await;
    if res.is_ok() {
        notify_referees_changed(&state);
    }
    res
}

#[tauri::command]
async fn submit_bout_panel(
    match_id: i64,
    tatami_number: i32,
    seats: Vec<models::BoutSeatInput>,
    judges: Option<i64>,
    expected_rev: Option<i64>,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.submit_bout_panel(match_id, tatami_number, seats, judges, expected_rev).await?;
    notify_referees_changed(&state);
    notify_bout_status(&state, match_id, "submitted", "submit", None);
    Ok(())
}

#[tauri::command]
async fn request_bout_approval(
    match_id: i64,
    tatami_number: i32,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::BoutApprovalState, String> {
    let db = state.db.lock().await;
    let res = db.request_bout_approval(match_id, tatami_number).await?;
    if res.required && res.status == "pending" {
        notify_bout_status(&state, match_id, "pending", "request", None);
    }
    Ok(res)
}

#[tauri::command]
async fn decide_bout_approval(
    match_id: i64,
    approve: bool,
    note: Option<String>,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let tatami = db.decide_bout_approval(match_id, approve, note.clone()).await?;
    let shown = note.as_deref().map(str::trim).filter(|n| !n.is_empty()).unwrap_or(if approve { "" } else { "Ditolak Chief Referee" });
    let (status, action) = if approve { ("approved", "approve") } else { ("draft", "reject") };
    let _ = state.server_broadcast.send(ArenaMessage {
        tatami_pk: 0,
        channel: "bracket".into(),
        payload: serde_json::json!({ "type": "bout_status", "match_id": match_id, "status": status, "action": action, "note": shown, "tatami": tatami }),
    });
    Ok(())
}

#[tauri::command]
async fn revise_bout_panel(
    match_id: i64,
    seats: Vec<models::BoutSeatInput>,
    judges: Option<i64>,
    note: Option<String>,
    expected_rev: Option<i64>,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let tatami = db.revise_bout_panel(match_id, seats, judges, note.clone(), expected_rev).await?;
    let shown = note.as_deref().map(str::trim).unwrap_or("").to_string();
    notify_referees_changed(&state);
    let _ = state.server_broadcast.send(ArenaMessage {
        tatami_pk: 0,
        channel: "bracket".into(),
        payload: serde_json::json!({ "type": "bout_status", "match_id": match_id, "status": "approved", "action": "revise", "note": shown, "tatami": tatami }),
    });
    Ok(())
}

#[tauri::command]
async fn ack_bout_revision(match_id: i64, state: tauri::State<'_, DesktopState>) -> Result<(), String> {
    let db = state.db.lock().await;
    db.ack_bout_revision(match_id).await
}

#[tauri::command]
async fn get_unseen_revisions(
    tatami_number: i32,
    event_id: Option<i64>,
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<models::RevisionNotice>, String> {
    let db = state.db.lock().await;
    db.get_unseen_revisions(tatami_number, event_id).await
}

#[tauri::command]
async fn get_referee_busy(event_id: i64, state: tauri::State<'_, DesktopState>) -> Result<Vec<models::RefereeBusy>, String> {
    let db = state.db.lock().await;
    db.get_referee_busy(event_id).await
}

#[tauri::command]
async fn get_bagan_bout_statuses(bagan_id: i64, state: tauri::State<'_, DesktopState>) -> Result<Vec<models::BoutStatusRow>, String> {
    let db = state.db.lock().await;
    db.get_bagan_bout_statuses(bagan_id).await
}

#[tauri::command]
async fn get_approval_queue(event_id: i64, state: tauri::State<'_, DesktopState>) -> Result<models::ApprovalQueue, String> {
    let db = state.db.lock().await;
    db.get_approval_queue(event_id).await
}

#[tauri::command]
async fn get_event_referee_settings(event_id: i64, state: tauri::State<'_, DesktopState>) -> Result<models::EventRefereeSettings, String> {
    let db = state.db.lock().await;
    db.get_event_referee_settings(event_id).await
}

#[tauri::command]
async fn set_event_referee_settings(
    event_id: i64,
    require_chief_approval: bool,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.set_event_referee_settings(event_id, require_chief_approval).await?;
    notify_referees_changed(&state);
    Ok(())
}

#[tauri::command]
async fn set_bout_judges(
    match_id: i64,
    tatami_number: i32,
    judges: i64,
    kata: bool,
    expected_rev: Option<i64>,
    state: tauri::State<'_, DesktopState>,
) -> Result<i64, String> {
    let db = state.db.lock().await;
    let res = db.set_bout_judges(match_id, tatami_number, judges, kata, expected_rev).await;
    if res.is_ok() {
        notify_referees_changed(&state);
    }
    res
}

#[tauri::command]
async fn reset_bout_panel(
    match_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let res = db.reset_bout_panel(match_id).await;
    if res.is_ok() {
        notify_referees_changed(&state);
    }
    res
}

#[tauri::command]
async fn export_tournament_excel(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<String, String> {
    let db = state.db.lock().await;
    let data = db.get_tournament_excel_data(event_id).await?;
    let bytes = crate::excel_exporter::generate_tournament_workbook(&data)
        .map_err(|e| format!("Gagal membuat workbook Excel: {}", e))?;

    let clean_name: String = data
        .event_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let file_name = format!("Rekap_Kejuaraan_{}.xlsx", clean_name);

    let downloads_dir = std::env::var("USERPROFILE")
        .map(|p| std::path::PathBuf::from(p).join("Downloads"))
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

    let file_path = downloads_dir.join(&file_name);
    std::fs::write(&file_path, bytes)
        .map_err(|e| format!("Gagal menyimpan file Excel ke {}: {}", file_path.display(), e))?;

    Ok(file_path.to_string_lossy().to_string())
}

#[tauri::command]
async fn get_marshalling_overview(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::MarshallingOverview, String> {
    let db = state.db.lock().await;
    db.get_marshalling_overview(event_id).await
}

#[tauri::command]
async fn broadcast_urgent_call(
    input: models::SubmitUrgentCallInput,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();

    let call_payload = serde_json::json!({
        "type": "urgent_call",
        "tatami_number": input.tatami_number,
        "athlete_name": input.athlete_name,
        "contingent": input.contingent.unwrap_or_else(|| "-".to_string()),
        "message": input.message,
        "timestamp": ts,
    });

    let _ = state.server_broadcast.send(server::ArenaMessage {
        tatami_pk: input.tatami_number as i64,
        channel: "marshalling".to_string(),
        payload: call_payload,
    });

    Ok(())
}

#[tauri::command]
async fn get_results_booklet_data(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::ResultsBookletData, String> {
    let db = state.db.lock().await;
    db.get_results_booklet_data(event_id).await
}

#[tauri::command]
async fn get_bob_candidates(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<models::BobSourceCategory>, String> {
    let db = state.db.lock().await;
    db.get_bob_candidates(event_id).await
}

#[tauri::command]
async fn create_bob_bagan(
    input: models::CreateBobBaganInput,
    state: tauri::State<'_, DesktopState>,
) -> Result<models::BobBaganResult, String> {
    let db = state.db.lock().await;
    let res = db.create_bob_bagan(input).await;
    if res.is_ok() {
        notify_bracket_changed(&state);
    }
    res
}

#[tauri::command]
async fn export_html_to_pdf(
    app: tauri::AppHandle,
    html_content: String,
    suggested_filename: Option<String>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    // 1. Locate msedge.exe on Windows
    let edge_paths = [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    ];
    let edge_exe = edge_paths
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .ok_or_else(|| "Microsoft Edge tidak ditemukan di sistem ini untuk rendering PDF.".to_string())?;

    // 2. Open Save File dialog
    let default_name = suggested_filename.unwrap_or_else(|| {
        format!(
            "Buku_Hasil_Turnamen_{}.pdf",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        )
    });

    let file = app
        .dialog()
        .file()
        .add_filter("PDF Document (*.pdf)", &["pdf"])
        .set_file_name(&default_name)
        .blocking_save_file();

    let Some(file_path) = file else {
        return Ok(None);
    };

    let target_path_buf = file_path.into_path().map_err(|e| e.to_string())?;
    let target_path = target_path_buf.to_string_lossy().to_string();

    // 3. Create temporary HTML file
    let temp_dir = std::env::temp_dir();
    let temp_file_name = format!(
        "ambrilindo_booklet_{}.html",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let temp_html_path = temp_dir.join(temp_file_name);
    std::fs::write(&temp_html_path, html_content)
        .map_err(|e| format!("Gagal menulis temporary HTML: {}", e))?;

    // 4. Ensure target directory exists
    if let Some(parent) = target_path_buf.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // 5. Run Edge headless print-to-pdf
    let file_url = format!("file:///{}", temp_html_path.to_string_lossy().replace('\\', "/"));
    let output = tokio::process::Command::new(edge_exe)
        .arg("--headless")
        .arg("--disable-gpu")
        .arg("--no-pdf-header-footer")
        .arg(format!("--print-to-pdf={}", target_path))
        .arg(&file_url)
        .output()
        .await
        .map_err(|e| format!("Gagal menjalankan Edge PDF printer: {}", e))?;

    // 6. Clean up temporary HTML
    let _ = std::fs::remove_file(&temp_html_path);

    if !target_path_buf.exists() {
        return Err(format!(
            "Edge selesai tanpa membuat file PDF. Output: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // 7. Open file with system default viewer
    let _ = std::process::Command::new("explorer").arg(&target_path).spawn();

    Ok(Some(target_path))
}


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (broadcast_tx, _rx) = broadcast::channel(server::BROADCAST_CAPACITY);
    let live_cache_arc = Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));

    let db_arc = Arc::new(Mutex::new(DatabaseManager::new()));
    let desktop_state = DesktopState {
        db: db_arc.clone(),
        audio: AudioService::new(),
        server_broadcast: broadcast_tx.clone(),
        live_tatami_cache: live_cache_arc.clone(),
    };

    let bcast_tx_for_setup = broadcast_tx.clone();
    let live_cache_for_setup = live_cache_arc.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let app_handle = app.handle().clone();
            sync::SyncService::spawn_sync_worker(db_arc, Some(app_handle));

            // License: verify the saved token offline, then quietly re-check online when possible
            // (picks up extensions, module changes and revocations; network errors are ignored).
            match app.path().app_config_dir() {
                Ok(dir) => {
                    license::init(dir);
                }
                Err(e) => eprintln!("[license] app config dir unavailable: {e}"),
            }
            let license_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(Some(st)) = license::refresh().await {
                    let _ = license_app.emit("license_changed", &st);
                } else {
                    let _ = license_app.emit("license_changed", &license::current());
                }
            });

            // Native IPC bridge: forward tatami_live_state from any webview to Axum server & cache
            let bcast_tx = bcast_tx_for_setup.clone();
            let cache_arc = live_cache_for_setup.clone();
            app.listen("tatami_live_state", move |event| {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(event.payload()) {
                    let data = val.get("payload").unwrap_or(&val).clone();
                    if let Some(t_num) = data.get("tatami").and_then(|v| v.as_i64()) {
                        let cache_clone = cache_arc.clone();
                        let data_clone = data.clone();
                        tokio::spawn(async move {
                            let mut cache = cache_clone.lock().await;
                            cache.insert(t_num, data_clone);
                        });

                        let _ = bcast_tx.send(server::ArenaMessage {
                            tatami_pk: t_num,
                            channel: "bracket".to_string(),
                            payload: serde_json::json!({
                                "type": "tatami_live_state",
                                "payload": data
                            }),
                        });
                    }
                }
            });

            Ok(())
        })
        .manage(desktop_state)
        .invoke_handler(tauri::generate_handler![
            get_license_status,
            activate_license,
            refresh_license,
            deactivate_license,
            get_network_info,
            check_ip_availability,
            start_arena_server,
            stop_arena_server,
            get_server_status,
            claim_tatami,
            release_tatami,
            pick_and_mount_database,
            mount_database_path,
            auto_mount_default_database,
            backup_database,
            backup_database_to_file,
            get_events,
            create_event,
            get_tatamis,
            create_tatami,
            delete_tatami,
            update_tatami_number,
            get_categories,
            get_drawing_eligible_categories,
            get_drawing_eligible_festival_categories,
            execute_drawing_bagan,
            execute_drawing_festival_bagan,
            delete_all_bagans,
            delete_all_festival_bagans,
            delete_category_bagans,
            delete_all_athletes,
            reset_total_tournament,
            delete_event,
            delete_all_categories,
            delete_all_contingents,
            delete_all_perguruans,
            get_bagans,
            get_bagan_by_id,
            update_bagan_layout_type,
            get_bagan_podium,
            get_athletes_for_bagan,
            update_bagan_details,
            get_detail_bagan,
            get_summary_juara,
            get_summary_bagan,
            save_match_result,
            open_control_panel_window,
            close_control_panel_window,
            get_media_data_url,
            get_athletes,
            get_enriched_athletes,
            create_athlete,
            update_athlete,
            delete_athlete,
            delete_athletes_batch,
            toggle_athlete_priority,
            get_perguruans,
            get_contingents,
            get_enriched_contingents,
            create_contingent,
            update_contingent,
            delete_contingent,
            get_enriched_perguruans,
            create_perguruan,
            update_perguruan,
            delete_perguruan,
            save_contingent_logo,
            get_team_match_info,
            get_match_juri_scores,
            assign_bout_athletes,
            add_team_bout,
            delete_team_bout,
            finish_team_match,
            play_buzzer,
            get_available_monitors,
            project_scoreboard,
            close_scoreboard,
            get_running_scoreboards,
            get_sync_config,
            get_sync_auth_status,
            set_sync_config,
            test_sync_connection,
            get_hosted_events,
            get_event_mapping,
            set_event_mapping,
            pull_athletes_from_hosted,
            push_bagan_to_hosted,
            force_sync_results_to_hosted,
            toggle_live_sync,
            get_sync_queue_stats,
            get_sync_queue_items,
            retry_sync_item,
            retry_all_failed_sync,
            mark_sync_item_synced,
            delete_sync_item,
            clear_synced_items,
            clear_all_sync_queue,
            open_festival_control_panel,
            get_festival_roster_data,
            get_festival_booklet_data,
            save_festival_roster_state,
            get_festival_format,
            set_festival_format,
            auto_pair_festival_category,
            mix_festival_categories,
            get_festival_mix_groups,
            save_festival_mix_groups,
            get_enriched_categories,
            create_category,
            update_category,
            toggle_category_vr,
            delete_category,
            get_category_athletes,
            get_timetable_editor_data,
            save_timetable_data,
            add_timetable_day,
            delete_timetable_day,
            reset_timetable,
            get_tatami_booklet_data,
            get_kop_surat,
            save_kop_surat,
            get_wasits,
            save_wasit,
            delete_wasit,
            get_tatami_panel_overview,
            assign_wasit_tatami,
            remove_wasit_tatami,
            clear_tatami_panel,
            auto_assign_tatami_panel,
            preview_wasit_import,
            import_wasits,
            save_wasit_photo,
            get_bout_panel,
            save_bout_panel,
            submit_bout_panel,
            request_bout_approval,
            decide_bout_approval,
            revise_bout_panel,
            ack_bout_revision,
            get_unseen_revisions,
            get_referee_busy,
            get_bagan_bout_statuses,
            get_approval_queue,
            get_event_referee_settings,
            set_event_referee_settings,
            set_bout_judges,
            reset_bout_panel,
            export_tournament_excel,
            get_marshalling_overview,
            broadcast_urgent_call,
            get_results_booklet_data,
            get_bob_candidates,
            create_bob_bagan,
            export_html_to_pdf
        ])
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed => {
                    use tauri::Manager;
                    let app = window.app_handle();
                    if window.label() == "main" {
                        for (label, win) in app.webview_windows() {
                            if label != "main" {
                                let _ = win.close();
                            }
                        }
                        app.exit(0);
                    } else if window.label().starts_with("control-panel-") {
                        let state = app.state::<DesktopState>();
                        let label = window.label().to_string();
                        let match_id_str = label.replace("control-panel-", "");
                        let match_id = match_id_str.parse::<i64>().unwrap_or(0);

                        let db_arc = state.db.clone();
                        let server_bcast = state.server_broadcast.clone();
                        let app_clone = app.clone();

                        tauri::async_runtime::spawn(async move {
                            let (pool_opt, _path) = {
                                let db = db_arc.lock().await;
                                (db.get_pool_cloned(), db.get_active_db_path_cloned())
                            };
                            if let Some(pool) = pool_opt {
                                let tatami_num: Option<i64> = if match_id > 0 {
                                    sqlx::query_scalar("SELECT tatami_number FROM backend_tatami WHERE detail_bagan_id = ? LIMIT 1")
                                        .bind(match_id)
                                        .fetch_optional(&pool)
                                        .await
                                        .unwrap_or(None)
                                } else {
                                    None
                                };

                                if match_id > 0 {
                                    let _ = sqlx::query("UPDATE backend_tatami SET detail_bagan_id = NULL WHERE detail_bagan_id = ?")
                                        .bind(match_id)
                                        .execute(&pool)
                                        .await;
                                    let _ = sqlx::query("UPDATE backend_detailbagan SET assigned_tatami_id = NULL WHERE id = ? AND selesai = 0")
                                        .bind(match_id)
                                        .execute(&pool)
                                        .await;
                                }

                                let close_payload = serde_json::json!({
                                    "tatami": tatami_num.unwrap_or(1),
                                    "matchId": match_id,
                                    "isFinished": true,
                                    "isClosed": true,
                                    "isTimerRunning": false
                                });
                                let _ = app_clone.emit("tatami_live_state", &close_payload);
                                let _ = app_clone.emit("bracket_updated", serde_json::json!({ "type": "bracket_updated" }));
                                let _ = server_bcast.send(crate::server::ArenaMessage {
                                    tatami_pk: 0,
                                    channel: "bracket".to_string(),
                                    payload: serde_json::json!({
                                        "type": "tatami_live_state",
                                        "payload": close_payload
                                    }),
                                });
                            }
                        });
                    }
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
