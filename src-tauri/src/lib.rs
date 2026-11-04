pub mod audio;
pub mod bracket;
pub mod db;
pub mod excel_exporter;
pub mod models;
pub mod referee;
pub mod server;
pub mod sync;

use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tauri::{Emitter, Manager};

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
    pub server_running: Mutex<bool>,
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
async fn check_ip_availability(
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<server::IpAvailabilityInfo>, String> {
    let is_running = *state.server_running.lock().await;
    Ok(server::check_ip_availability(is_running).await)
}

#[tauri::command]
async fn stop_arena_server(
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    server::stop_server().await?;
    let mut running = state.server_running.lock().await;
    *running = false;
    Ok(())
}

#[tauri::command]
async fn start_arena_server(
    app: tauri::AppHandle,
    selected_ip: Option<String>,
    state: tauri::State<'_, DesktopState>,
) -> Result<NetworkInfo, String> {
    let mut running = state.server_running.lock().await;
    if *running {
        return Ok(server::get_network_info_with_ip(selected_ip.as_deref()));
    }

    let app_state = Arc::new(server::AppState {
        broadcast_tx: state.server_broadcast.clone(),
        db: state.db.clone(),
        app_handle: Some(app),
    });

    server::start_server(app_state).await?;
    *running = true;
    Ok(server::get_network_info_with_ip(selected_ip.as_deref()))
}

#[tauri::command]
async fn get_server_status(
    state: tauri::State<'_, DesktopState>,
) -> Result<bool, String> {
    Ok(*state.server_running.lock().await)
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
    db.execute_drawing_bagan(event_id, category_ids, &shuffle_type, vr_category_ids).await
}

#[tauri::command]
async fn delete_all_bagans(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.delete_all_bagans(event_id).await
}

#[tauri::command]
async fn delete_category_bagans(
    event_id: i64,
    category_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.delete_category_bagans(event_id, category_id).await
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
async fn get_bagans(
    event_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<Bagan>, String> {
    let db = state.db.lock().await;
    db.get_bagans(event_id).await
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
    let (pool_opt, db_path_opt) = {
        db.save_match_result(input).await?;
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

    Ok(())
}

#[tauri::command]
async fn open_control_panel_window(
    state: tauri::State<'_, DesktopState>,
    app: tauri::AppHandle,
    match_id: i64,
    bagan_id: i64,
    tatami: Option<i64>,
) -> Result<(), String> {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

    let win_label = format!("control-panel-{}", match_id);
    let t_num = tatami.unwrap_or(1);

    // Notify match is running on this tatami and enqueue to sync
    let t_num_i32 = t_num as i32;
    if match_id > 0 {
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
            format!("/control-panel?matchId=0&baganId=0&isFestival=true&tatami={}", t_num),
            format!("Ambrilindo - Control Panel Festival (Bagan 000) - Tatami {}", t_num),
        )
    } else {
        (
            format!("/control-panel?matchId={}&baganId={}&tatami={}", match_id, bagan_id, t_num),
            format!("Ambrilindo - Control Panel Partai #{}", match_id),
        )
    };

    let _win = WebviewWindowBuilder::new(&app, &win_label, WebviewUrl::App(url.into()))
        .title(title)
        .inner_size(1366.0, 820.0)
        .min_inner_size(1024.0, 700.0)
        .center()
        .build()
        .map_err(|e| format!("Gagal membuka window control panel: {}", e))?;

    Ok(())
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
) -> Result<(), String> {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

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

    let url = format!("/control-panel?{}", query);
    let title = format!("Ambrilindo - Control Panel Festival (Bagan 000) - Tatami {}", t_num);

    let _win = WebviewWindowBuilder::new(&app, &win_label, WebviewUrl::App(url.into()))
        .title(title)
        .inner_size(1366.0, 820.0)
        .min_inner_size(1024.0, 700.0)
        .center()
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
    db.finish_team_match(parent_match_id, winner_override).await
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

    let url = format!("/scoreboard?tatami={}", tatami_number);
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
    db.save_wasit(input).await
}

#[tauri::command]
async fn delete_wasit(
    wasit_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.delete_wasit(wasit_id).await
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
    db.assign_wasit_tatami(input).await.map(|_| ())
}

#[tauri::command]
async fn remove_wasit_tatami(
    assignment_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.remove_wasit_tatami(assignment_id).await
}

#[tauri::command]
async fn clear_tatami_panel(
    tatami_id: i64,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.clear_tatami_panel(tatami_id).await
}

#[tauri::command]
async fn auto_assign_tatami_panel(
    input: models::AutoAssignPanelInput,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.auto_assign_tatami_panel(input).await
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
    db.create_bob_bagan(input).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (broadcast_tx, _rx) = broadcast::channel(100);

    let db_arc = Arc::new(Mutex::new(DatabaseManager::new()));
    let desktop_state = DesktopState {
        db: db_arc.clone(),
        audio: AudioService::new(),
        server_broadcast: broadcast_tx,
        server_running: Mutex::new(false),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let app_handle = app.handle().clone();
            sync::SyncService::spawn_sync_worker(db_arc, Some(app_handle));
            Ok(())
        })
        .manage(desktop_state)
        .invoke_handler(tauri::generate_handler![
            get_network_info,
            check_ip_availability,
            start_arena_server,
            stop_arena_server,
            get_server_status,
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
            execute_drawing_bagan,
            delete_all_bagans,
            delete_category_bagans,
            delete_all_athletes,
            get_bagans,
            get_bagan_by_id,
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
            save_festival_roster_state,
            auto_pair_festival_category,
            mix_festival_categories,
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
            export_tournament_excel,
            get_marshalling_overview,
            broadcast_urgent_call,
            get_results_booklet_data,
            get_bob_candidates,
            create_bob_bagan
        ])
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed => {
                    if window.label() == "main" {
                        use tauri::Manager;
                        let app = window.app_handle();
                        for (label, win) in app.webview_windows() {
                            if label != "main" {
                                let _ = win.close();
                            }
                        }
                        app.exit(0);
                    }
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
