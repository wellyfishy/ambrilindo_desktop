use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use qrcode::render::svg;
use qrcode::QrCode;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Emitter;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing::{error, info};

use axum::http::StatusCode;
use tokio::sync::Mutex;

use crate::models::*;

pub const DEFAULT_PORT: u16 = 8000;

#[derive(Clone)]
pub struct AppState {
    pub broadcast_tx: broadcast::Sender<ArenaMessage>,
    pub db: Arc<Mutex<crate::db::DatabaseManager>>,
    pub app_handle: Option<tauri::AppHandle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArenaMessage {
    pub tatami_pk: i64,
    pub channel: String, // "scoring-board", "jury-panel", "admin-control"
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterfaceInfo {
    pub name: String,
    pub ip: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub port: u16,
    pub local_ip: String,
    pub server_url: String,
    pub qr_svg: String,
    pub available_ips: Vec<NetworkInterfaceInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub app: String,
    pub version: String,
    pub port: u16,
    pub local_ip: String,
}

pub fn get_local_ip() -> String {
    match local_ip_address::local_ip() {
        Ok(ip) => ip.to_string(),
        Err(_) => "127.0.0.1".to_string(),
    }
}

pub fn get_available_ips() -> Vec<NetworkInterfaceInfo> {
    let default_ip = get_local_ip();
    let mut result = Vec::new();

    if let Ok(netifas) = local_ip_address::list_afinet_netifas() {
        for (name, ip) in netifas {
            if let IpAddr::V4(ipv4) = ip {
                if !ipv4.is_loopback() && !ipv4.is_unspecified() {
                    let ip_str = ipv4.to_string();
                    if !result.iter().any(|item: &NetworkInterfaceInfo| item.ip == ip_str) {
                        result.push(NetworkInterfaceInfo {
                            name,
                            ip: ip_str.clone(),
                            is_default: ip_str == default_ip,
                        });
                    }
                }
            }
        }
    }

    if result.is_empty() {
        result.push(NetworkInterfaceInfo {
            name: "Localhost".into(),
            ip: "127.0.0.1".into(),
            is_default: true,
        });
    }

    result
}

pub fn generate_qr_svg(url: &str) -> String {
    match QrCode::new(url.as_bytes()) {
        Ok(code) => code
            .render()
            .min_dimensions(200, 200)
            .dark_color(svg::Color("#020617"))
            .light_color(svg::Color("#ffffff"))
            .build(),
        Err(_) => String::new(),
    }
}

pub fn get_network_info_with_ip(custom_ip: Option<&str>) -> NetworkInfo {
    let available_ips = get_available_ips();
    let default_ip = get_local_ip();

    let selected_ip = if let Some(ip) = custom_ip {
        if !ip.trim().is_empty() {
            ip.trim().to_string()
        } else {
            default_ip
        }
    } else {
        default_ip
    };

    let url = format!("http://{}:{}", selected_ip, DEFAULT_PORT);
    let qr_svg = generate_qr_svg(&url);

    NetworkInfo {
        port: DEFAULT_PORT,
        local_ip: selected_ip,
        server_url: url,
        qr_svg,
        available_ips,
    }
}

pub fn get_network_info() -> NetworkInfo {
    get_network_info_with_ip(None)
}

async fn health_check() -> Json<HealthStatus> {
    Json(HealthStatus {
        status: "ok".into(),
        app: "Ambrilindo Desktop".into(),
        version: "3.0.0".into(),
        port: DEFAULT_PORT,
        local_ip: get_local_ip(),
    })
}

async fn network_info_handler() -> Json<NetworkInfo> {
    Json(get_network_info())
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Path((channel, tatami_pk)): Path<(String, i64)>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, channel, tatami_pk, state))
}

async fn handle_socket(
    mut socket: WebSocket,
    channel: String,
    tatami_pk: i64,
    state: Arc<AppState>,
) {
    let mut rx = state.broadcast_tx.subscribe();
    info!("WebSocket client connected for channel '{}' on Tatami #{}", channel, tatami_pk);

    loop {
        tokio::select! {
            // Receive inbound message from client
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                            let arena_msg = ArenaMessage {
                                tatami_pk,
                                channel: channel.clone(),
                                payload: parsed,
                            };
                            let _ = state.broadcast_tx.send(arena_msg.clone());
                            if let Some(app) = &state.app_handle {
                                let _ = app.emit("tatami_remote_input", &arena_msg);
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        info!("WebSocket client disconnected: {} (Tatami #{})", channel, tatami_pk);
                        break;
                    }
                    _ => {}
                }
            }
            // Broadcast outbound message to client
            broadcast_result = rx.recv() => {
                if let Ok(msg) = broadcast_result {
                    if msg.tatami_pk == tatami_pk {
                        let text = serde_json::to_string(&msg.payload).unwrap_or_default();
                        if socket.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    }
}

async fn list_events_api(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_events().await {
        Ok(events) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": events }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn list_tatamis_api(Path(event_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_tatamis(event_id).await {
        Ok(tatamis) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": tatamis }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn list_bagans_api(Path(event_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_bagans(event_id).await {
        Ok(bagans) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": bagans }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn detail_bagan_api(Path(bagan_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_detail_bagan(bagan_id).await {
        Ok(details) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": details }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn list_athletes_api(Path(event_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_athletes(event_id).await {
        Ok(athletes) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": athletes }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn list_contingents_api(Path(event_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_contingents(event_id).await {
        Ok(contingents) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": contingents }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn tatami_current_match_api(
    Path(tatami_num): Path<i32>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_tatami_current_match(tatami_num).await {
        Ok(m) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": m }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn tatami_matches_api(
    Path(tatami_num): Path<i32>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_tatami_matches(tatami_num).await {
        Ok(matches) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": matches }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct SaveKataInput {
    pub match_id: i64,
    pub tatami: Option<i64>,
    pub kata1: Option<String>,
    pub kata2: Option<String>,
}

async fn save_match_kata_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveKataInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    if let Err(e) = db.save_match_kata(payload.match_id, payload.kata1.clone(), payload.kata2.clone()).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response();
    }

    let tatami_pk = payload.tatami.unwrap_or(1);
    let arena_msg = ArenaMessage {
        tatami_pk,
        channel: "lo-kata".into(),
        payload: serde_json::json!({
            "type": "lo_kata_broadcast",
            "tatami": tatami_pk,
            "match_id": payload.match_id,
            "kata_aka": payload.kata1,
            "kata_ao": payload.kata2,
        }),
    };

    let _ = state.broadcast_tx.send(arena_msg.clone());
    if let Some(app) = &state.app_handle {
        let _ = app.emit("tatami_remote_input", &arena_msg);
    }

    (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response()
}

#[derive(Debug, Deserialize)]
pub struct SubmitJudgeScoreInput {
    pub tatami: i64,
    pub juri: i32,
    pub score_aka: Option<String>,
    pub score_ao: Option<String>,
    pub flag: Option<String>,
}

async fn submit_judge_score_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SubmitJudgeScoreInput>,
) -> impl IntoResponse {
    let arena_msg = ArenaMessage {
        tatami_pk: payload.tatami,
        channel: "jury-panel".into(),
        payload: serde_json::json!({
            "type": "judge_score",
            "tatami": payload.tatami,
            "juri": payload.juri,
            "score_aka": payload.score_aka,
            "score_ao": payload.score_ao,
            "flag": payload.flag,
        }),
    };

    let _ = state.broadcast_tx.send(arena_msg.clone());
    if let Some(app) = &state.app_handle {
        let _ = app.emit("tatami_remote_input", &arena_msg);
    }

    (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response()
}


fn get_i64_val(payload: &serde_json::Value, keys: &[&str]) -> Option<i64> {
    for k in keys {
        if let Some(v) = payload.get(*k) {
            if let Some(n) = v.as_i64() { return Some(n); }
            if let Some(s) = v.as_str() { if let Ok(n) = s.parse::<i64>() { return Some(n); } }
        }
    }
    None
}

fn get_i32_val(payload: &serde_json::Value, keys: &[&str]) -> Option<i32> {
    for k in keys {
        if let Some(v) = payload.get(*k) {
            if let Some(n) = v.as_i64() { return Some(n as i32); }
            if let Some(s) = v.as_str() { if let Ok(n) = s.parse::<i32>() { return Some(n); } }
        }
    }
    None
}

fn get_str_val(payload: &serde_json::Value, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(v) = payload.get(*k) {
            if let Some(s) = v.as_str() { return Some(s.to_string()); }
        }
    }
    None
}

fn get_bool_val(payload: &serde_json::Value, keys: &[&str]) -> Option<bool> {
    for k in keys {
        if let Some(v) = payload.get(*k) {
            if let Some(b) = v.as_bool() { return Some(b); }
            if let Some(s) = v.as_str() {
                if s == "true" || s == "1" { return Some(true); }
                if s == "false" || s == "0" { return Some(false); }
            }
        }
    }
    None
}

#[derive(Debug, Deserialize)]
pub struct CreateEventHttpInput {
    pub nama_event: String,
    pub kode_event: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTatamiHttpInput {
    pub event_id: i64,
    pub tatami_number: i32,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTatamiNumberHttpInput {
    pub tatami_id: i64,
    pub new_number: i32,
}

#[derive(Debug, Deserialize)]
pub struct IdHttpInput {
    pub id: i64,
}

#[derive(Debug, Deserialize)]
pub struct TatamiIdHttpInput {
    pub tatami_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAthleteHttpInput {
    pub id: i64,
    pub input: AthleteMutationInput,
}

#[derive(Debug, Deserialize)]
pub struct BatchIdsHttpInput {
    pub ids: Vec<i64>,
}

#[derive(Debug, Deserialize)]
pub struct TogglePriorityHttpInput {
    pub id: i64,
    pub is_priority: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBaganDetailsHttpInput {
    pub bagan_id: i64,
    pub nama_bagan: Option<String>,
    pub matchups: Vec<UpdateMatchAthletesInput>,
}

#[derive(Debug, Deserialize)]
pub struct ExecuteDrawingHttpInput {
    pub event_id: i64,
    pub category_ids: Vec<i64>,
    pub shuffle_type: Option<String>,
    pub vr_category_ids: Option<Vec<i64>>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteCategoryBagansHttpInput {
    pub event_id: i64,
    pub category_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct ToggleCategoryVrHttpInput {
    pub category_id: i64,
    pub has_vr: bool,
}

#[derive(Debug, Deserialize)]
pub struct SaveTimetableHttpInput {
    pub event_id: i64,
    pub payload: SaveTimetablePayload,
}

#[derive(Debug, Deserialize)]
pub struct EventIdHttpInput {
    pub event_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct DayIdHttpInput {
    pub day_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct SaveFestivalHttpInput {
    pub event_id: i64,
    pub state: FestivalRosterState,
}

#[derive(Debug, Deserialize)]
pub struct AutoPairFestivalHttpInput {
    pub event_id: i64,
    pub nomor_tanding_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct MixCategoriesHttpInput {
    pub event_id: i64,
    pub source_ids: Vec<i64>,
    pub target_id: Option<i64>,
    pub new_name: String,
    pub delete_sources: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateContingentHttpInput {
    pub event_id: i64,
    pub nama_utusan: String,
    pub logo: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateContingentHttpInput {
    pub id: i64,
    pub nama_utusan: String,
    pub logo: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePerguruanHttpInput {
    pub event_id: i64,
    pub nama_perguruan: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePerguruanHttpInput {
    pub id: i64,
    pub nama_perguruan: String,
}

#[derive(Debug, Deserialize)]
pub struct SaveLogoHttpInput {
    pub file_name: String,
    pub base64_data: String,
}

#[derive(Debug, Deserialize)]
pub struct SaveKopSuratHttpInput {
    pub event_id: i64,
    pub nama_organisasi: String,
    pub alamat: String,
    pub kontak: String,
    pub logo: Option<String>,
}

async fn perform_match_save_effects(
    state: &AppState,
    pool_opt: Option<sqlx::Pool<sqlx::Sqlite>>,
    db_path_opt: Option<std::path::PathBuf>,
) {
    if let Some(pool) = pool_opt {
        let app_handle = state.app_handle.clone();
        tokio::spawn(async move {
            if let Ok((synced, failed)) = crate::sync::SyncService::process_sync_queue(&pool, db_path_opt.as_ref()).await {
                if synced > 0 || failed > 0 {
                    if let Some(app) = app_handle {
                        let _ = app.emit("sync_queue_changed", serde_json::json!({
                            "synced": synced,
                            "failed": failed
                        }));
                    }
                }
            }
        });
    }

    if let Some(app) = &state.app_handle {
        let _ = app.emit("bracket_updated", ());
    }

    let _ = state.broadcast_tx.send(ArenaMessage {
        tatami_pk: 0,
        channel: "bracket".into(),
        payload: serde_json::json!({ "type": "bracket_updated" }),
    });
}

async fn list_enriched_athletes_api(Path(event_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_enriched_athletes(event_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn list_categories_api(Path(event_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_categories(event_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn list_enriched_categories_api(Path(event_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_enriched_categories(event_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn list_category_athletes_api(Path(category_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_category_athletes(category_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn drawing_eligible_categories_api(Path(event_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_drawing_eligible_categories(event_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn list_perguruans_api(Path(event_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_perguruans(event_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn get_bagan_api(Path(bagan_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_bagan_by_id(bagan_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn get_bagan_podium_api(Path(bagan_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_bagan_podium(bagan_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn athletes_for_bagan_api(Path(bagan_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_athletes_for_bagan(bagan_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn summary_juara_api(Path(event_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_summary_juara(event_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn summary_bagan_api(Path(event_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_summary_bagan(event_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn get_timetable_api(Path(event_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_timetable_editor_data(event_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn get_festival_roster_api(Path(event_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_festival_roster_data(event_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn tatami_booklet_api(
    Path((event_id, day_id, tatami_id)): Path<(i64, i64, i64)>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_tatami_booklet_data(event_id, day_id, tatami_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn get_kop_surat_api(Path(event_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_kop_surat(event_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn create_event_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateEventHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.create_event(&payload.nama_event, payload.kode_event).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn create_tatami_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateTatamiHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.create_tatami(payload.event_id, payload.tatami_number).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn update_tatami_number_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateTatamiNumberHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.update_tatami_number(payload.tatami_id, payload.new_number).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn delete_tatami_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TatamiIdHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.delete_tatami(payload.tatami_id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn create_athlete_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AthleteMutationInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.create_athlete(payload).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn update_athlete_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateAthleteHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.update_athlete(payload.id, payload.input).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn delete_athlete_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<IdHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.delete_athlete(payload.id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn delete_athletes_batch_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<BatchIdsHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.delete_athletes_batch(payload.ids).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn toggle_athlete_priority_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TogglePriorityHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.toggle_athlete_priority(payload.id, payload.is_priority).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn save_match_result_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveMatchInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    let res = db.save_match_result(payload).await;
    let (pool_opt, db_path_opt) = (db.get_pool_cloned(), db.get_active_db_path_cloned());
    drop(db);

    match res {
        Ok(()) => {
            perform_match_save_effects(&state, pool_opt, db_path_opt).await;
            (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn update_bagan_details_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateBaganDetailsHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.update_bagan_details(payload.bagan_id, payload.nama_bagan, payload.matchups).await {
        Ok(()) => {
            if let Some(app) = &state.app_handle {
                let _ = app.emit("bracket_updated", ());
            }
            let _ = state.broadcast_tx.send(ArenaMessage {
                tatami_pk: 0,
                channel: "bracket".into(),
                payload: serde_json::json!({ "type": "bracket_updated" }),
            });
            (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn execute_drawing_bagan_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExecuteDrawingHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    let shuffle = payload.shuffle_type.unwrap_or_else(|| "random".into());
    let vr_cats = payload.vr_category_ids.unwrap_or_default();
    match db.execute_drawing_bagan(payload.event_id, payload.category_ids, &shuffle, vr_cats).await {
        Ok(v) => {
            if let Some(app) = &state.app_handle {
                let _ = app.emit("bracket_updated", ());
            }
            (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn delete_category_bagans_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<DeleteCategoryBagansHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.delete_category_bagans(payload.event_id, payload.category_id).await {
        Ok(()) => {
            if let Some(app) = &state.app_handle {
                let _ = app.emit("bracket_updated", ());
            }
            (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn delete_all_bagans_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EventIdHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.delete_all_bagans(payload.event_id).await {
        Ok(()) => {
            if let Some(app) = &state.app_handle {
                let _ = app.emit("bracket_updated", ());
            }
            (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn delete_all_athletes_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EventIdHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.delete_all_athletes(payload.event_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn create_category_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CategoryMutationInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.create_category(payload).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn update_category_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CategoryMutationInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.update_category(payload).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn toggle_category_vr_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ToggleCategoryVrHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.toggle_category_vr(payload.category_id, payload.has_vr).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn delete_category_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<IdHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.delete_category(payload.id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn save_timetable_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveTimetableHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.save_timetable_data(payload.event_id, payload.payload).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn add_timetable_day_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EventIdHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.add_timetable_day(payload.event_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn delete_timetable_day_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<DayIdHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.delete_timetable_day(payload.day_id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn reset_timetable_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EventIdHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.reset_timetable_data(payload.event_id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn save_festival_roster_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveFestivalHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.save_festival_roster_state(payload.event_id, payload.state).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn auto_pair_festival_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AutoPairFestivalHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.auto_pair_festival_category(payload.event_id, payload.nomor_tanding_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn mix_festival_categories_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MixCategoriesHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    let del_sources = payload.delete_sources.unwrap_or(false);
    match db.mix_festival_categories(payload.event_id, payload.source_ids, payload.target_id, payload.new_name, del_sources).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn save_kop_surat_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveKopSuratHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.save_kop_surat(payload.event_id, payload.nama_organisasi, payload.alamat, payload.kontak, payload.logo).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}


async fn list_enriched_contingents_api(Path(event_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_enriched_contingents(event_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn create_contingent_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateContingentHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.create_contingent(payload.event_id, payload.nama_utusan, payload.logo).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn update_contingent_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateContingentHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.update_contingent(payload.id, payload.nama_utusan, payload.logo).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn delete_contingent_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<IdHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.delete_contingent(payload.id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn list_enriched_perguruans_api(Path(event_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_enriched_perguruans(event_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn create_perguruan_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreatePerguruanHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.create_perguruan(payload.event_id, payload.nama_perguruan).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn update_perguruan_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdatePerguruanHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.update_perguruan(payload.id, payload.nama_perguruan).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn delete_perguruan_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<IdHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.delete_perguruan(payload.id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn save_contingent_logo_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveLogoHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.save_contingent_logo(&payload.file_name, &payload.base64_data).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn dispatch_command_api(
    Path(cmd): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match cmd.as_str() {
        "get_events" => match db.get_events().await {
            Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
        },
        "create_event" => {
            let nama = get_str_val(&payload, &["namaEvent", "nama_event", "nama"]).unwrap_or_default();
            let kode = get_str_val(&payload, &["kodeEvent", "kode_event", "kode"]);
            match db.create_event(&nama, kode).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_tatamis" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_tatamis(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "create_tatami" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            let t_num = get_i32_val(&payload, &["tatamiNumber", "tatami_number", "number"]).unwrap_or(1);
            match db.create_tatami(ev_id, t_num).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "delete_tatami" => {
            let t_id = get_i64_val(&payload, &["tatamiId", "tatami_id", "id"]).unwrap_or(0);
            match db.delete_tatami(t_id).await {
                Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "update_tatami_number" => {
            let t_id = get_i64_val(&payload, &["tatamiId", "tatami_id", "id"]).unwrap_or(0);
            let num = get_i32_val(&payload, &["newNumber", "new_number"]).unwrap_or(1);
            match db.update_tatami_number(t_id, num).await {
                Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_categories" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_categories(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_enriched_categories" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_enriched_categories(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "create_category" => {
            match serde_json::from_value::<CategoryMutationInput>(payload.get("input").unwrap_or(&payload).clone()) {
                Ok(inp) => match db.create_category(inp).await {
                    Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                    Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
                },
                Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "status": "error", "message": format!("Invalid payload: {}", e) }))).into_response(),
            }
        },
        "update_category" => {
            match serde_json::from_value::<CategoryMutationInput>(payload.get("input").unwrap_or(&payload).clone()) {
                Ok(inp) => match db.update_category(inp).await {
                    Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                    Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
                },
                Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "status": "error", "message": format!("Invalid payload: {}", e) }))).into_response(),
            }
        },
        "toggle_category_vr" => {
            let cat_id = get_i64_val(&payload, &["categoryId", "category_id", "id"]).unwrap_or(0);
            let has_vr = get_bool_val(&payload, &["hasVr", "has_vr"]).unwrap_or(false);
            match db.toggle_category_vr(cat_id, has_vr).await {
                Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "delete_category" => {
            let cat_id = get_i64_val(&payload, &["categoryId", "category_id", "id"]).unwrap_or(0);
            match db.delete_category(cat_id).await {
                Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_category_athletes" => {
            let cat_id = get_i64_val(&payload, &["categoryId", "category_id", "id"]).unwrap_or(0);
            match db.get_category_athletes(cat_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_drawing_eligible_categories" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_drawing_eligible_categories(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "execute_drawing_bagan" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            let cat_ids = serde_json::from_value::<Vec<i64>>(payload.get("categoryIds").or(payload.get("category_ids")).cloned().unwrap_or(serde_json::Value::Array(vec![]))).unwrap_or_default();
            let shuffle_type = get_str_val(&payload, &["shuffleType", "shuffle_type"]).unwrap_or_else(|| "random".into());
            let vr_cat_ids = serde_json::from_value::<Vec<i64>>(payload.get("vrCategoryIds").or(payload.get("vr_category_ids")).cloned().unwrap_or(serde_json::Value::Array(vec![]))).unwrap_or_default();
            match db.execute_drawing_bagan(ev_id, cat_ids, &shuffle_type, vr_cat_ids).await {
                Ok(v) => {
                    if let Some(app) = &state.app_handle {
                        let _ = app.emit("bracket_updated", ());
                    }
                    (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response()
                },
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "delete_all_bagans" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.delete_all_bagans(ev_id).await {
                Ok(()) => {
                    if let Some(app) = &state.app_handle {
                        let _ = app.emit("bracket_updated", ());
                    }
                    (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response()
                },
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "delete_category_bagans" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            let cat_id = get_i64_val(&payload, &["categoryId", "category_id"]).unwrap_or(0);
            match db.delete_category_bagans(ev_id, cat_id).await {
                Ok(()) => {
                    if let Some(app) = &state.app_handle {
                        let _ = app.emit("bracket_updated", ());
                    }
                    (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response()
                },
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "delete_all_athletes" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.delete_all_athletes(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_bagans" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_bagans(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_bagan_by_id" => {
            let b_id = get_i64_val(&payload, &["baganId", "bagan_id", "id"]).unwrap_or(0);
            match db.get_bagan_by_id(b_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_bagan_podium" => {
            let b_id = get_i64_val(&payload, &["baganId", "bagan_id", "id"]).unwrap_or(0);
            match db.get_bagan_podium(b_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_detail_bagan" => {
            let b_id = get_i64_val(&payload, &["baganId", "bagan_id", "id"]).unwrap_or(0);
            match db.get_detail_bagan(b_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_athletes_for_bagan" => {
            let b_id = get_i64_val(&payload, &["baganId", "bagan_id", "id"]).unwrap_or(0);
            match db.get_athletes_for_bagan(b_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "update_bagan_details" => {
            let b_id = get_i64_val(&payload, &["baganId", "bagan_id"]).unwrap_or(0);
            let nama = get_str_val(&payload, &["namaBagan", "nama_bagan"]);
            let matchups = serde_json::from_value::<Vec<UpdateMatchAthletesInput>>(payload.get("matchups").cloned().unwrap_or(serde_json::Value::Array(vec![]))).unwrap_or_default();
            match db.update_bagan_details(b_id, nama, matchups).await {
                Ok(()) => {
                    if let Some(app) = &state.app_handle {
                        let _ = app.emit("bracket_updated", ());
                    }
                    let _ = state.broadcast_tx.send(ArenaMessage {
                        tatami_pk: 0,
                        channel: "bracket".into(),
                        payload: serde_json::json!({ "type": "bracket_updated" }),
                    });
                    (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response()
                },
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_summary_juara" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_summary_juara(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_summary_bagan" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_summary_bagan(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "save_match_result" => {
            match serde_json::from_value::<SaveMatchInput>(payload.get("input").unwrap_or(&payload).clone()) {
                Ok(inp) => {
                    let res = db.save_match_result(inp).await;
                    let (pool_opt, db_path_opt) = (db.get_pool_cloned(), db.get_active_db_path_cloned());
                    drop(db);
                    match res {
                        Ok(()) => {
                            perform_match_save_effects(&state, pool_opt, db_path_opt).await;
                            (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response()
                        },
                        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
                    }
                },
                Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "status": "error", "message": format!("Invalid payload: {}", e) }))).into_response(),
            }
        },
        "get_athletes" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_athletes(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_enriched_athletes" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_enriched_athletes(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "create_athlete" => {
            match serde_json::from_value::<AthleteMutationInput>(payload.get("input").unwrap_or(&payload).clone()) {
                Ok(inp) => match db.create_athlete(inp).await {
                    Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                    Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
                },
                Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "status": "error", "message": format!("Invalid payload: {}", e) }))).into_response(),
            }
        },
        "update_athlete" => {
            let a_id = get_i64_val(&payload, &["id"]).unwrap_or(0);
            match serde_json::from_value::<AthleteMutationInput>(payload.get("input").unwrap_or(&payload).clone()) {
                Ok(inp) => match db.update_athlete(a_id, inp).await {
                    Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                    Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
                },
                Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "status": "error", "message": format!("Invalid payload: {}", e) }))).into_response(),
            }
        },
        "delete_athlete" => {
            let a_id = get_i64_val(&payload, &["id"]).unwrap_or(0);
            match db.delete_athlete(a_id).await {
                Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "delete_athletes_batch" => {
            let ids = serde_json::from_value::<Vec<i64>>(payload.get("ids").cloned().unwrap_or(serde_json::Value::Array(vec![]))).unwrap_or_default();
            match db.delete_athletes_batch(ids).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "toggle_athlete_priority" => {
            let a_id = get_i64_val(&payload, &["id"]).unwrap_or(0);
            let is_p = get_bool_val(&payload, &["isPriority", "is_priority"]).unwrap_or(false);
            match db.toggle_athlete_priority(a_id, is_p).await {
                Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        
        "get_enriched_contingents" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_enriched_contingents(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "create_contingent" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            let nama = get_str_val(&payload, &["namaUtusan", "nama_utusan", "nama"]).unwrap_or_default();
            let logo = get_str_val(&payload, &["logo"]);
            match db.create_contingent(ev_id, nama, logo).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "update_contingent" => {
            let id = get_i64_val(&payload, &["id"]).unwrap_or(0);
            let nama = get_str_val(&payload, &["namaUtusan", "nama_utusan", "nama"]).unwrap_or_default();
            let logo = get_str_val(&payload, &["logo"]);
            match db.update_contingent(id, nama, logo).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "delete_contingent" => {
            let id = get_i64_val(&payload, &["id"]).unwrap_or(0);
            match db.delete_contingent(id).await {
                Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_enriched_perguruans" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_enriched_perguruans(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "create_perguruan" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            let nama = get_str_val(&payload, &["namaPerguruan", "nama_perguruan", "nama"]).unwrap_or_default();
            match db.create_perguruan(ev_id, nama).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "update_perguruan" => {
            let id = get_i64_val(&payload, &["id"]).unwrap_or(0);
            let nama = get_str_val(&payload, &["namaPerguruan", "nama_perguruan", "nama"]).unwrap_or_default();
            match db.update_perguruan(id, nama).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "delete_perguruan" => {
            let id = get_i64_val(&payload, &["id"]).unwrap_or(0);
            match db.delete_perguruan(id).await {
                Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "save_contingent_logo" => {
            let file_name = get_str_val(&payload, &["fileName", "file_name"]).unwrap_or_else(|| "logo.png".into());
            let base64_data = get_str_val(&payload, &["base64Data", "base64_data"]).unwrap_or_default();
            match db.save_contingent_logo(&file_name, &base64_data).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_perguruans" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_perguruans(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_contingents" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_contingents(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_timetable_editor_data" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_timetable_editor_data(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "save_timetable_data" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match serde_json::from_value::<SaveTimetablePayload>(payload.get("payload").unwrap_or(&payload).clone()) {
                Ok(p) => match db.save_timetable_data(ev_id, p).await {
                    Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
                    Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
                },
                Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "status": "error", "message": format!("Invalid payload: {}", e) }))).into_response(),
            }
        },
        "add_timetable_day" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.add_timetable_day(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "delete_timetable_day" => {
            let d_id = get_i64_val(&payload, &["dayId", "day_id"]).unwrap_or(0);
            match db.delete_timetable_day(d_id).await {
                Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "reset_timetable" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.reset_timetable_data(ev_id).await {
                Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_festival_roster_data" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_festival_roster_data(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "save_festival_roster_state" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match serde_json::from_value::<FestivalRosterState>(payload.get("state").unwrap_or(&payload).clone()) {
                Ok(s) => match db.save_festival_roster_state(ev_id, s).await {
                    Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
                    Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
                },
                Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "status": "error", "message": format!("Invalid payload: {}", e) }))).into_response(),
            }
        },
        "auto_pair_festival_category" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            let nt_id = get_i64_val(&payload, &["nomorTandingId", "nomor_tanding_id"]);
            match db.auto_pair_festival_category(ev_id, nt_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "mix_festival_categories" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            let source_ids = serde_json::from_value::<Vec<i64>>(payload.get("sourceIds").or(payload.get("source_ids")).cloned().unwrap_or(serde_json::Value::Array(vec![]))).unwrap_or_default();
            let target_id = get_i64_val(&payload, &["targetId", "target_id"]);
            let name = get_str_val(&payload, &["newName", "new_name"]).unwrap_or_default();
            let del_sources = get_bool_val(&payload, &["deleteSources", "delete_sources"]).unwrap_or(false);
            match db.mix_festival_categories(ev_id, source_ids, target_id, name, del_sources).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_kop_surat" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_kop_surat(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "save_kop_surat" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            let org = get_str_val(&payload, &["namaOrganisasi", "nama_organisasi"]).unwrap_or_default();
            let almt = get_str_val(&payload, &["alamat"]).unwrap_or_default();
            let kontak = get_str_val(&payload, &["kontak"]).unwrap_or_default();
            let logo = get_str_val(&payload, &["logo"]);
            match db.save_kop_surat(ev_id, org, almt, kontak, logo).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_tatami_booklet_data" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            let d_id = get_i64_val(&payload, &["dayId", "day_id"]).unwrap_or(1);
            let t_id = get_i64_val(&payload, &["tatamiId", "tatami_id"]).unwrap_or(1);
            match db.get_tatami_booklet_data(ev_id, d_id, t_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        unknown => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "status": "error", "message": format!("Unknown command: {}", unknown) }))).into_response(),
    }
}

pub fn find_frontend_build_dir() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("build"),
        PathBuf::from("../build"),
        PathBuf::from("../../build"),
    ];
    for c in &candidates {
        if c.exists() && c.join("index.html").exists() {
            return c.canonicalize().ok().or_else(|| Some(c.clone()));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let exe_candidates = [
                parent.join("build"),
                parent.join("../build"),
                parent.join("../../build"),
            ];
            for c in &exe_candidates {
                if c.exists() && c.join("index.html").exists() {
                    return c.canonicalize().ok().or_else(|| Some(c.clone()));
                }
            }
        }
    }
    None
}

async fn media_file_api(
    Path(file_path): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    if let Some(media_dir) = db.get_media_dir() {
        let clean_path = file_path.trim_start_matches('/').trim_start_matches('\\');
        let full_path = media_dir.join(clean_path);
        if full_path.exists() && full_path.is_file() {
            if let Ok(bytes) = tokio::fs::read(&full_path).await {
                let mime = if clean_path.ends_with(".png") {
                    "image/png"
                } else if clean_path.ends_with(".webp") {
                    "image/webp"
                } else if clean_path.ends_with(".jpg") || clean_path.ends_with(".jpeg") {
                    "image/jpeg"
                } else if clean_path.ends_with(".svg") {
                    "image/svg+xml"
                } else {
                    "application/octet-stream"
                };
                return (
                    StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, mime)],
                    bytes,
                ).into_response();
            }
        }
    }
    (StatusCode::NOT_FOUND, "Media not found").into_response()
}

pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let mut router = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/network-info", get(network_info_handler))
        // Event routes
        .route("/api/events", get(list_events_api))
        .route("/api/events/create", post(create_event_api))
        // Tatami routes
        .route("/api/tatamis/:event_id", get(list_tatamis_api))
        .route("/api/tatami/:tatami_number/current-match", get(tatami_current_match_api))
        .route("/api/tatami/:tatami_number/matches", get(tatami_matches_api))
        .route("/api/tatami/create", post(create_tatami_api))
        .route("/api/tatami/update-number", post(update_tatami_number_api))
        .route("/api/tatami/delete", post(delete_tatami_api))
        // Bagan routes
        .route("/api/bagans/:event_id", get(list_bagans_api))
        .route("/api/bagan/:bagan_id", get(get_bagan_api))
        .route("/api/bagan-podium/:bagan_id", get(get_bagan_podium_api))
        .route("/api/detail-bagan/:bagan_id", get(detail_bagan_api))
        .route("/api/bagan/update-details", post(update_bagan_details_api))
        .route("/api/bagan/execute-drawing", post(execute_drawing_bagan_api))
        .route("/api/bagan/delete-category", post(delete_category_bagans_api))
        .route("/api/bagan/delete-all", post(delete_all_bagans_api))
        // Athlete routes
        .route("/api/athletes/:event_id", get(list_athletes_api))
        .route("/api/enriched-athletes/:event_id", get(list_enriched_athletes_api))
        .route("/api/athletes/create", post(create_athlete_api))
        .route("/api/athletes/update", post(update_athlete_api))
        .route("/api/athletes/delete", post(delete_athlete_api))
        .route("/api/athletes/delete-batch", post(delete_athletes_batch_api))
        .route("/api/athletes/toggle-priority", post(toggle_athlete_priority_api))
        .route("/api/athletes/delete-all", post(delete_all_athletes_api))
        // Category routes
        .route("/api/categories/:event_id", get(list_categories_api))
        .route("/api/enriched-categories/:event_id", get(list_enriched_categories_api))
        .route("/api/category-athletes/:category_id", get(list_category_athletes_api))
        .route("/api/drawing-eligible-categories/:event_id", get(drawing_eligible_categories_api))
        .route("/api/categories/create", post(create_category_api))
        .route("/api/categories/update", post(update_category_api))
        .route("/api/categories/toggle-vr", post(toggle_category_vr_api))
        .route("/api/categories/delete", post(delete_category_api))
        // Contingent & Perguruan routes
        .route("/api/contingents/:event_id", get(list_contingents_api))
        .route("/api/enriched-contingents/:event_id", get(list_enriched_contingents_api))
        .route("/api/contingents/create", post(create_contingent_api))
        .route("/api/contingents/update", post(update_contingent_api))
        .route("/api/contingents/delete", post(delete_contingent_api))
        .route("/api/contingents/upload-logo", post(save_contingent_logo_api))
        .route("/api/enriched-perguruans/:event_id", get(list_enriched_perguruans_api))
        .route("/api/perguruans/create", post(create_perguruan_api))
        .route("/api/perguruans/update", post(update_perguruan_api))
        .route("/api/perguruans/delete", post(delete_perguruan_api))

        .route("/api/perguruans/:event_id", get(list_perguruans_api))
        .route("/api/athletes-for-bagan/:bagan_id", get(athletes_for_bagan_api))
        // Match results
        .route("/api/match/save-result", post(save_match_result_api))
        .route("/api/match/save-kata", post(save_match_kata_api))
        .route("/api/tatami/submit-judge-score", post(submit_judge_score_api))
        // Standings & Reports
        .route("/api/summary-juara/:event_id", get(summary_juara_api))
        .route("/api/summary-bagan/:event_id", get(summary_bagan_api))
        .route("/api/tatami-booklet/:event_id/:day_id/:tatami_id", get(tatami_booklet_api))
        .route("/api/kop-surat/:event_id", get(get_kop_surat_api))
        .route("/api/kop-surat/save", post(save_kop_surat_api))
        // Timetable routes
        .route("/api/timetable/:event_id", get(get_timetable_api))
        .route("/api/timetable/save", post(save_timetable_api))
        .route("/api/timetable/add-day", post(add_timetable_day_api))
        .route("/api/timetable/delete-day", post(delete_timetable_day_api))
        .route("/api/timetable/reset", post(reset_timetable_api))
        // Festival routes
        .route("/api/festival/:event_id", get(get_festival_roster_api))
        .route("/api/festival/save-state", post(save_festival_roster_api))
        .route("/api/festival/auto-pair", post(auto_pair_festival_api))
        .route("/api/festival/mix-categories", post(mix_festival_categories_api))
        // Universal RPC Dispatcher
        .route("/api/command/:cmd", post(dispatch_command_api))
        // Static and WebSocket
        .route("/media/*file_path", get(media_file_api))
        .route("/ws/:channel/:tatami_pk", get(ws_handler));

    if let Some(build_dir) = find_frontend_build_dir() {
        let index_file = build_dir.join("index.html");
        info!("Serving venue mobile web frontend from {:?}", build_dir);
        let serve_dir = ServeDir::new(&build_dir).fallback(ServeFile::new(index_file));
        router = router.fallback_service(serve_dir);
    } else {
        info!("Frontend build directory not found, serving API only");
    }

    router
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Starts the embedded Axum server strictly on port 8000.
/// If port 8000 is occupied, halts and returns Err describing the port conflict.
pub async fn start_server(
    state: Arc<AppState>,
) -> Result<(SocketAddr, tokio::task::JoinHandle<()>), String> {
    let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), DEFAULT_PORT);

    let listener = match TcpListener::bind(bind_addr).await {
        Ok(l) => l,
        Err(e) => {
            let err_msg = format!(
                "Room tidak dapat dimulai — Port {} sedang digunakan oleh aplikasi lain ({})",
                DEFAULT_PORT, e
            );
            error!("{}", err_msg);
            return Err(err_msg);
        }
    };

    let local_addr = listener
        .local_addr()
        .unwrap_or(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), DEFAULT_PORT));

    info!(
        "Ambrilindo Arena Server bound successfully on http://0.0.0.0:{}",
        DEFAULT_PORT
    );

    let app = create_router(state);
    let handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            error!("Server error: {}", e);
        }
    });

    Ok((local_addr, handle))
}
