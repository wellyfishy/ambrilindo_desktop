pub mod audio;
pub mod bracket;
pub mod db;
pub mod models;
pub mod server;
pub mod sync;

use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tauri::Manager;

use audio::{AudioService, SoundType};
use db::DatabaseManager;
use models::{
    AthleteMutationInput, Atlet, Bagan, DrawingCategoryInfo, DrawingSummary, EnrichedAtlet,
    EnrichedDetailBagan, Event, NomorTanding, Perguruan, SaveMatchInput, Tatami, Utusan,
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
async fn start_arena_server(
    state: tauri::State<'_, DesktopState>,
) -> Result<NetworkInfo, String> {
    let mut running = state.server_running.lock().await;
    if *running {
        return Ok(server::get_network_info());
    }

    let app_state = Arc::new(server::AppState {
        broadcast_tx: state.server_broadcast.clone(),
        db: state.db.clone(),
    });

    server::start_server(app_state).await?;
    *running = true;
    Ok(server::get_network_info())
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
async fn save_match_result(
    input: SaveMatchInput,
    state: tauri::State<'_, DesktopState>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    db.save_match_result(input).await
}

#[tauri::command]
async fn open_control_panel_window(
    app: tauri::AppHandle,
    match_id: i64,
    bagan_id: i64,
) -> Result<(), String> {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

    let win_label = format!("control-panel-{}", match_id);

    // If window already exists, bring it forward and set focus
    if let Some(win) = app.get_webview_window(&win_label) {
        let _ = win.show();
        let _ = win.set_focus();
        return Ok(());
    }

    let url = format!("/control-panel?matchId={}&baganId={}", match_id, bagan_id);
    let _win = WebviewWindowBuilder::new(&app, &win_label, WebviewUrl::App(url.into()))
        .title(format!("Ambrilindo - Control Panel Partai #{}", match_id))
        .inner_size(1366.0, 820.0)
        .min_inner_size(1024.0, 700.0)
        .center()
        .build()
        .map_err(|e| format!("Gagal membuka window control panel: {}", e))?;

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
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    let monitors = app.available_monitors().map_err(|e| e.to_string())?;
    let monitor = monitors.get(monitor_id).ok_or_else(|| "Monitor tidak ditemukan".to_string())?;

    let win_label = format!("scoreboard-tatami-{}", tatami_number);

    // If window already exists, bring it forward and update position
    if let Some(win) = app.get_webview_window(&win_label) {
        let _ = win.set_position(tauri::Position::Physical(*monitor.position()));
        let _ = win.set_fullscreen(true);
        let _ = win.set_focus();
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

    Ok(())
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (broadcast_tx, _rx) = broadcast::channel(100);

    let desktop_state = DesktopState {
        db: Arc::new(Mutex::new(DatabaseManager::new())),
        audio: AudioService::new(),
        server_broadcast: broadcast_tx,
        server_running: Mutex::new(false),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(desktop_state)
        .invoke_handler(tauri::generate_handler![
            get_network_info,
            start_arena_server,
            get_server_status,
            pick_and_mount_database,
            mount_database_path,
            auto_mount_default_database,
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
            get_detail_bagan,
            save_match_result,
            open_control_panel_window,
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
            play_buzzer,
            get_available_monitors,
            project_scoreboard,
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
            clear_all_sync_queue
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
