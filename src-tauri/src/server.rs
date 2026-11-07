use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        DefaultBodyLimit, Path, State,
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
use tracing::{error, info, warn};

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
pub struct IpAvailabilityInfo {
    pub ip: String,
    pub name: String,
    pub status: String, // "available", "occupied", "active_event", "current_host"
    pub event_name: Option<String>,
    pub is_default: bool,
    pub is_current_host: bool,
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

/// The one running Axum server, if any. This is the single source of truth for
/// "is the LAN server running" — IPC commands and the LAN dispatcher both read it,
/// so the state can no longer drift between them.
struct RunningServer {
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
    handle: tokio::task::JoinHandle<()>,
}

static RUNNING_SERVER: Mutex<Option<RunningServer>> = Mutex::const_new(None);

/// Bumped on every stop so open WebSocket loops exit instead of keeping the
/// old server (and port 8000) alive.
static WS_STOP: std::sync::OnceLock<tokio::sync::watch::Sender<u64>> = std::sync::OnceLock::new();

fn ws_stop() -> &'static tokio::sync::watch::Sender<u64> {
    WS_STOP.get_or_init(|| tokio::sync::watch::channel(0u64).0)
}

/// Capacity of the arena broadcast channel. Live state is sent ~7x/second per active
/// tatami, so this leaves several seconds of headroom for a slow device before it
/// starts skipping messages.
pub const BROADCAST_CAPACITY: usize = 1024;

pub async fn is_server_running() -> bool {
    RUNNING_SERVER
        .lock()
        .await
        .as_ref()
        .map(|r| !r.handle.is_finished())
        .unwrap_or(false)
}

/// Stops the LAN server and waits until port 8000 is actually released, so an
/// immediate restart does not hit "port in use".
pub async fn stop_server() -> Result<(), String> {
    let running = RUNNING_SERVER.lock().await.take();
    let Some(running) = running else {
        return Ok(());
    };

    let _ = running.shutdown_tx.send(());
    ws_stop().send_modify(|v| *v = v.wrapping_add(1));
    info!("Arena server shutdown signal sent; closing WebSocket clients");

    let mut handle = running.handle;
    match tokio::time::timeout(std::time::Duration::from_secs(3), &mut handle).await {
        Ok(_) => info!("Arena server stopped cleanly"),
        Err(_) => {
            warn!("Arena server did not stop within 3s; aborting it");
            handle.abort();
            let _ = handle.await;
        }
    }
    Ok(())
}

pub async fn check_ip_availability(is_our_server_running: bool) -> Vec<IpAvailabilityInfo> {
    let mut results = Vec::new();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(350))
        .build()
        .unwrap_or_default();

    // Query active event name from local server if our server is running or reachable
    let mut local_active_event: Option<String> = None;
    let local_probe = format!("http://127.0.0.1:{}/api/events", DEFAULT_PORT);
    if let Ok(ev_resp) = client.get(&local_probe).send().await {
        if let Ok(ev_json) = ev_resp.json::<serde_json::Value>().await {
            if let Some(arr) = ev_json.get("data").and_then(|d| d.as_array()) {
                if let Some(first_ev) = arr.first() {
                    if let Some(name) = first_ev.get("nama_event").and_then(|n| n.as_str()) {
                        local_active_event = Some(name.to_string());
                    }
                }
            }
        }
    }

    // 1. Check 0.0.0.0 (All Interfaces)
    let (zero_status, zero_is_curr) = if is_our_server_running {
        ("current_host".to_string(), true)
    } else {
        match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", DEFAULT_PORT)).await {
            Ok(_) => ("available".to_string(), false),
            Err(_) => {
                if local_active_event.is_some() {
                    ("active_event".to_string(), false)
                } else {
                    ("occupied".to_string(), false)
                }
            }
        }
    };

    results.push(IpAvailabilityInfo {
        ip: "0.0.0.0".to_string(),
        name: "Semua Interface Jaringan (Default)".to_string(),
        status: zero_status,
        event_name: local_active_event.clone(),
        is_default: true,
        is_current_host: zero_is_curr,
    });

    // 2. Check each local IP
    let available_ips = get_available_ips();
    for iface in available_ips {
        if iface.ip == "127.0.0.1" || iface.ip == "0.0.0.0" {
            continue;
        }

        let mut status = "available".to_string();
        let mut event_name = None;
        let mut is_curr = false;

        if is_our_server_running {
            // Server this app is running binds 0.0.0.0:8000, so it answers on this interface
            status = "current_host".to_string();
            event_name = local_active_event.clone();
            is_curr = true;
        } else {
            // Try HTTP probe first to see if another Ambrilindo server is already active on this IP
            let probe_url = format!("http://{}:{}/api/health", iface.ip, DEFAULT_PORT);
            if let Ok(resp) = client.get(&probe_url).send().await {
                if resp.status().is_success() {
                    if let Ok(health) = resp.json::<HealthStatus>().await {
                        if health.status == "ok" {
                            status = "active_event".to_string();
                            // Query active event name
                            let events_url = format!("http://{}:{}/api/events", iface.ip, DEFAULT_PORT);
                            if let Ok(ev_resp) = client.get(&events_url).send().await {
                                if let Ok(ev_json) = ev_resp.json::<serde_json::Value>().await {
                                    if let Some(arr) = ev_json.get("data").and_then(|d| d.as_array()) {
                                        if let Some(first_ev) = arr.first() {
                                            if let Some(name) = first_ev.get("nama_event").and_then(|n| n.as_str()) {
                                                event_name = Some(name.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // If not responding via HTTP, test socket bind
            if status != "active_event" {
                let addr = format!("{}:{}", iface.ip, DEFAULT_PORT);
                match tokio::net::TcpListener::bind(&addr).await {
                    Ok(_) => {
                        status = "available".to_string();
                    }
                    Err(_) => {
                        status = "occupied".to_string();
                    }
                }
            }
        }

        results.push(IpAvailabilityInfo {
            ip: iface.ip.clone(),
            name: iface.name.clone(),
            status,
            event_name,
            is_default: iface.is_default,
            is_current_host: is_curr,
        });
    }

    results
}

async fn ip_availability_api() -> impl IntoResponse {
    let list = check_ip_availability(true).await;
    (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": list })))
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
    let mut stop_rx = ws_stop().subscribe();
    let socket_id = NEXT_SOCKET_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    info!("WebSocket client connected for channel '{}' on Tatami #{}", channel, tatami_pk);

    loop {
        tokio::select! {
            // Server is stopping: close this client so the old server can shut down.
            _ = stop_rx.changed() => {
                let _ = socket.send(Message::Close(None)).await;
                break;
            }
            // Receive inbound message from client
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(mut parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                            let (target_tatami, from_host_ui) = route_inbound(&parsed, tatami_pk);
                            if let Some(obj) = parsed.as_object_mut() {
                                // Tag with the sender so it is not echoed back to itself.
                                obj.insert(ORIGIN_KEY.into(), serde_json::json!(socket_id));
                            }
                            let arena_msg = ArenaMessage {
                                tatami_pk: target_tatami,
                                channel: channel.clone(),
                                payload: parsed.clone(),
                            };
                            let _ = state.broadcast_tx.send(arena_msg.clone());
                            // The host's own windows already got this via Tauri IPC, so
                            // only forward messages coming from other devices.
                            if !from_host_ui {
                                if let Some(app) = &state.app_handle {
                                    let _ = app.emit("tatami_remote_input", &arena_msg);
                                    if let Some(msg_type) = parsed.get("type").and_then(|t| t.as_str()) {
                                        if msg_type == "tatami_live_state" {
                                            let data_payload = parsed.get("payload").unwrap_or(&parsed);
                                            let _ = app.emit("tatami_live_state", data_payload);
                                        } else if msg_type == "bracket_updated" {
                                            let _ = app.emit("bracket_updated", &parsed);
                                        }
                                    }
                                }
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
                match broadcast_result {
                    Ok(msg) => {
                        if !should_deliver(&msg, tatami_pk, socket_id) {
                            continue;
                        }
                        let text = serde_json::to_string(&msg.payload).unwrap_or_default();
                        if socket.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!(
                            "WebSocket client '{}' (Tatami #{}) is too slow; skipped {} messages",
                            channel, tatami_pk, skipped
                        );
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

static NEXT_SOCKET_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Key added to relayed payloads naming the socket that sent them.
const ORIGIN_KEY: &str = "_origin";

/// Decides which tatami an inbound WebSocket message belongs to, and whether it came
/// from the host's own UI (which must not be re-emitted to the host's windows).
///
/// Live state is sent by every control panel on the shared `/ws/bracket/0` socket.
/// Re-targeting it to its own tatami means tablets on `/ws/<role>/<n>` only receive
/// their own tatami's traffic instead of every tatami's.
fn route_inbound(parsed: &serde_json::Value, socket_tatami: i64) -> (i64, bool) {
    let from_host_ui = parsed.get("origin").and_then(|o| o.as_str()) == Some("host");
    if socket_tatami != 0 {
        return (socket_tatami, from_host_ui);
    }
    let is_live_state = parsed.get("type").and_then(|t| t.as_str()) == Some("tatami_live_state");
    if is_live_state {
        let inner = parsed.get("payload").unwrap_or(parsed);
        if let Some(t) = inner.get("tatami").and_then(|t| t.as_i64()) {
            if t > 0 {
                return (t, from_host_ui);
            }
        }
    }
    (0, from_host_ui)
}

/// Overview sockets (tatami 0: bracket view, marshalling, master data) receive
/// everything; a tatami socket receives its own tatami plus global (0) messages.
/// Nobody receives its own message back.
fn should_deliver(msg: &ArenaMessage, socket_tatami: i64, socket_id: u64) -> bool {
    if msg.payload.get(ORIGIN_KEY).and_then(|o| o.as_u64()) == Some(socket_id) {
        return false;
    }
    socket_tatami == 0 || msg.tatami_pk == 0 || msg.tatami_pk == socket_tatami
}

#[cfg(test)]
mod ws_routing_tests {
    use super::*;

    fn msg(tatami_pk: i64, payload: serde_json::Value) -> ArenaMessage {
        ArenaMessage { tatami_pk, channel: "test".into(), payload }
    }

    #[test]
    fn live_state_on_overview_socket_is_retargeted() {
        let p = serde_json::json!({ "type": "tatami_live_state", "payload": { "tatami": 3 } });
        assert_eq!(route_inbound(&p, 0), (3, false));
        let host = serde_json::json!({ "type": "tatami_live_state", "origin": "host", "payload": { "tatami": 2 } });
        assert_eq!(route_inbound(&host, 0), (2, true));
        // Other messages on the overview socket stay global.
        let other = serde_json::json!({ "type": "bracket_updated", "tatami": 4 });
        assert_eq!(route_inbound(&other, 0), (0, false));
        // Messages on a tatami socket keep that socket's tatami.
        let jury = serde_json::json!({ "type": "judge_score", "tatami": 9 });
        assert_eq!(route_inbound(&jury, 1), (1, false));
    }

    #[test]
    fn delivery_rules() {
        let t2 = msg(2, serde_json::json!({ "type": "x" }));
        assert!(should_deliver(&t2, 2, 10));
        assert!(!should_deliver(&t2, 1, 10));
        assert!(should_deliver(&t2, 0, 10));
        let global = msg(0, serde_json::json!({ "type": "x" }));
        assert!(should_deliver(&global, 5, 10));
        let own = msg(2, serde_json::json!({ "type": "x", "_origin": 10 }));
        assert!(!should_deliver(&own, 2, 10));
        assert!(should_deliver(&own, 2, 11));
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
pub struct VrChallengeInput {
    pub tatami: i64,
    pub side: String, // "aka" | "ao"
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct VrResolveInput {
    pub tatami: i64,
    pub side: String, // "aka" | "ao"
    pub decision: String, // "accepted" | "rejected" | "cancelled"
}

async fn submit_vr_challenge_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<VrChallengeInput>,
) -> impl IntoResponse {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let arena_msg = ArenaMessage {
        tatami_pk: payload.tatami,
        channel: "vr".into(),
        payload: serde_json::json!({
            "type": "vr_challenge_started",
            "tatami": payload.tatami,
            "side": payload.side,
            "reason": payload.reason,
            "timestamp": now,
        }),
    };

    let _ = state.broadcast_tx.send(arena_msg.clone());
    if let Some(app) = &state.app_handle {
        let _ = app.emit("tatami_remote_input", &arena_msg);
    }

    (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response()
}

async fn resolve_vr_challenge_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<VrResolveInput>,
) -> impl IntoResponse {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let arena_msg = ArenaMessage {
        tatami_pk: payload.tatami,
        channel: "vr".into(),
        payload: serde_json::json!({
            "type": "vr_challenge_resolved",
            "tatami": payload.tatami,
            "side": payload.side,
            "decision": payload.decision,
            "timestamp": now,
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

async fn drawing_eligible_festival_categories_api(Path(event_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_drawing_eligible_festival_categories(event_id).await {
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

async fn get_repechage_info_api(Path(bagan_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_repechage_info(bagan_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn generate_repechage_api(State(state): State<Arc<AppState>>, Json(payload): Json<serde_json::Value>) -> impl IntoResponse {
    let db = state.db.lock().await;
    let main_bagan_id = get_i64_val(&payload, &["mainBaganId", "main_bagan_id", "baganId", "bagan_id", "id"]).unwrap_or(0);
    match db.generate_repechage_bracket(main_bagan_id).await {
        Ok(v) => {
            if let Some(app) = &state.app_handle {
                let _ = app.emit("bracket_updated", ());
            }
            (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn get_round_robin_data_api(Path(bagan_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_round_robin_data(bagan_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn create_round_robin_bagan_api(State(state): State<Arc<AppState>>, Json(payload): Json<serde_json::Value>) -> impl IntoResponse {
    let db = state.db.lock().await;
    let event_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(0);
    let nomor_tanding_id = get_i64_val(&payload, &["nomorTandingId", "nomor_tanding_id", "categoryId", "category_id"]).unwrap_or(0);
    match db.create_round_robin_bagan(event_id, nomor_tanding_id).await {
        Ok(v) => {
            if let Some(app) = &state.app_handle {
                let _ = app.emit("bracket_updated", ());
            }
            (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response()
        },
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

async fn get_festival_booklet_api(Path(event_id): Path<i64>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_festival_booklet_data(event_id, None).await {
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

async fn execute_drawing_festival_bagan_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExecuteDrawingHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    let shuffle = payload.shuffle_type.unwrap_or_else(|| "random".into());
    match db.execute_drawing_festival_bagan(payload.event_id, payload.category_ids, &shuffle).await {
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

async fn delete_all_festival_bagans_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EventIdHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.delete_all_festival_bagans(payload.event_id).await {
        Ok(count) => {
            if let Some(app) = &state.app_handle {
                let _ = app.emit("bracket_updated", ());
            }
            (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": count }))).into_response()
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
        Ok(v) => {
            let _ = state.broadcast_tx.send(ArenaMessage {
                tatami_pk: 0,
                channel: "master-data".into(),
                payload: serde_json::json!({ "type": "contingent_updated", "event_id": payload.event_id }),
            });
            if let Some(app) = &state.app_handle {
                let _ = app.emit("master_data_updated", &serde_json::json!({ "type": "contingent_updated", "event_id": payload.event_id }));
            }
            (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn update_contingent_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateContingentHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.update_contingent(payload.id, payload.nama_utusan, payload.logo).await {
        Ok(v) => {
            let _ = state.broadcast_tx.send(ArenaMessage {
                tatami_pk: 0,
                channel: "master-data".into(),
                payload: serde_json::json!({ "type": "contingent_updated" }),
            });
            if let Some(app) = &state.app_handle {
                let _ = app.emit("master_data_updated", &serde_json::json!({ "type": "contingent_updated" }));
            }
            (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn delete_contingent_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<IdHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.delete_contingent(payload.id).await {
        Ok(()) => {
            let _ = state.broadcast_tx.send(ArenaMessage {
                tatami_pk: 0,
                channel: "master-data".into(),
                payload: serde_json::json!({ "type": "contingent_updated" }),
            });
            if let Some(app) = &state.app_handle {
                let _ = app.emit("master_data_updated", &serde_json::json!({ "type": "contingent_updated" }));
            }
            (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response()
        },
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
        Ok(v) => {
            let _ = state.broadcast_tx.send(ArenaMessage {
                tatami_pk: 0,
                channel: "master-data".into(),
                payload: serde_json::json!({ "type": "perguruan_updated", "event_id": payload.event_id }),
            });
            if let Some(app) = &state.app_handle {
                let _ = app.emit("master_data_updated", &serde_json::json!({ "type": "perguruan_updated", "event_id": payload.event_id }));
            }
            (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn update_perguruan_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdatePerguruanHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.update_perguruan(payload.id, payload.nama_perguruan).await {
        Ok(v) => {
            let _ = state.broadcast_tx.send(ArenaMessage {
                tatami_pk: 0,
                channel: "master-data".into(),
                payload: serde_json::json!({ "type": "perguruan_updated" }),
            });
            if let Some(app) = &state.app_handle {
                let _ = app.emit("master_data_updated", &serde_json::json!({ "type": "perguruan_updated" }));
            }
            (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response()
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn delete_perguruan_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<IdHttpInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.delete_perguruan(payload.id).await {
        Ok(()) => {
            let _ = state.broadcast_tx.send(ArenaMessage {
                tatami_pk: 0,
                channel: "master-data".into(),
                payload: serde_json::json!({ "type": "perguruan_updated" }),
            });
            if let Some(app) = &state.app_handle {
                let _ = app.emit("master_data_updated", &serde_json::json!({ "type": "perguruan_updated" }));
            }
            (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response()
        },
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
        "check_ip_availability" => {
            let list = check_ip_availability(true).await;
            (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": list }))).into_response()
        },
        // Only the host laptop itself may stop its server (via Tauri IPC). A LAN
        // device stopping it would cut off every other operator and tablet.
        "stop_arena_server" | "start_arena_server" => (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "status": "error", "message": "Server hanya bisa dinyalakan/dihentikan dari laptop host." })),
        ).into_response(),
        "claim_tatami" => {
            let tatami = get_i64_val(&payload, &["tatami", "tatamiNumber", "tatami_number"]).unwrap_or(0);
            let owner_id = get_str_val(&payload, &["ownerId", "owner_id"]).unwrap_or_default();
            let owner_label = get_str_val(&payload, &["ownerLabel", "owner_label"]).unwrap_or_default();
            let force = get_bool_val(&payload, &["force"]).unwrap_or(false);
            if tatami <= 0 || owner_id.is_empty() {
                (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "status": "error", "message": "tatami dan ownerId wajib diisi" }))).into_response()
            } else {
                let res = crate::tatami_claim::claim(tatami, &owner_id, &owner_label, force);
                (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": res }))).into_response()
            }
        },
        "release_tatami" => {
            let tatami = get_i64_val(&payload, &["tatami", "tatamiNumber", "tatami_number"]).unwrap_or(0);
            let owner_id = get_str_val(&payload, &["ownerId", "owner_id"]).unwrap_or_default();
            let released = crate::tatami_claim::release(tatami, &owner_id);
            (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": released }))).into_response()
        },
        // Sent by an operator-tambahan laptop when it opens a match locally, so the
        // host's DB records which match is running on which tatami.
        "notify_match_running" => {
            let match_id = get_i64_val(&payload, &["matchId", "match_id"]).unwrap_or(0);
            let tatami = get_i32_val(&payload, &["tatami", "tatamiNumber", "tatami_number"]).unwrap_or(1);
            if match_id <= 0 {
                (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": null }))).into_response()
            } else {
                match db.notify_match_running(match_id, tatami).await {
                    Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": null }))).into_response(),
                    Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
                }
            }
        },
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
        "get_drawing_eligible_festival_categories" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_drawing_eligible_festival_categories(ev_id).await {
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
        "execute_drawing_festival_bagan" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            let cat_ids = serde_json::from_value::<Vec<i64>>(payload.get("categoryIds").or(payload.get("category_ids")).cloned().unwrap_or(serde_json::Value::Array(vec![]))).unwrap_or_default();
            let shuffle_type = get_str_val(&payload, &["shuffleType", "shuffle_type"]).unwrap_or_else(|| "random".into());
            match db.execute_drawing_festival_bagan(ev_id, cat_ids, &shuffle_type).await {
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
        "delete_all_festival_bagans" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.delete_all_festival_bagans(ev_id).await {
                Ok(count) => {
                    if let Some(app) = &state.app_handle {
                        let _ = app.emit("bracket_updated", ());
                    }
                    (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": count }))).into_response()
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
        "update_bagan_layout_type" => {
            let b_id = get_i64_val(&payload, &["baganId", "bagan_id", "id"]).unwrap_or(0);
            let lt = payload.get("layoutType").or(payload.get("layout_type")).and_then(|v| v.as_str()).unwrap_or("quadrant").to_string();
            match db.update_bagan_layout_type(b_id, lt).await {
                Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
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
        "get_repechage_info" => {
            let b_id = get_i64_val(&payload, &["baganId", "bagan_id", "id"]).unwrap_or(0);
            match db.get_repechage_info(b_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "generate_repechage_bracket" => {
            let main_bagan_id = get_i64_val(&payload, &["mainBaganId", "main_bagan_id", "baganId", "bagan_id", "id"]).unwrap_or(0);
            match db.generate_repechage_bracket(main_bagan_id).await {
                Ok(v) => {
                    if let Some(app) = &state.app_handle {
                        let _ = app.emit("bracket_updated", ());
                    }
                    (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response()
                },
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_round_robin_data" => {
            let b_id = get_i64_val(&payload, &["baganId", "bagan_id", "id"]).unwrap_or(0);
            match db.get_round_robin_data(b_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "create_round_robin_bagan" => {
            let event_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(0);
            let nomor_tanding_id = get_i64_val(&payload, &["nomorTandingId", "nomor_tanding_id", "categoryId", "category_id"]).unwrap_or(0);
            match db.create_round_robin_bagan(event_id, nomor_tanding_id).await {
                Ok(v) => {
                    if let Some(app) = &state.app_handle {
                        let _ = app.emit("bracket_updated", ());
                    }
                    (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response()
                },
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
                Ok(v) => {
                    let _ = state.broadcast_tx.send(ArenaMessage {
                        tatami_pk: 0,
                        channel: "master-data".into(),
                        payload: serde_json::json!({ "type": "contingent_updated", "event_id": ev_id }),
                    });
                    if let Some(app) = &state.app_handle {
                        let _ = app.emit("master_data_updated", &serde_json::json!({ "type": "contingent_updated", "event_id": ev_id }));
                    }
                    (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response()
                },
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "update_contingent" => {
            let id = get_i64_val(&payload, &["id"]).unwrap_or(0);
            let nama = get_str_val(&payload, &["namaUtusan", "nama_utusan", "nama"]).unwrap_or_default();
            let logo = get_str_val(&payload, &["logo"]);
            match db.update_contingent(id, nama, logo).await {
                Ok(v) => {
                    let _ = state.broadcast_tx.send(ArenaMessage {
                        tatami_pk: 0,
                        channel: "master-data".into(),
                        payload: serde_json::json!({ "type": "contingent_updated" }),
                    });
                    if let Some(app) = &state.app_handle {
                        let _ = app.emit("master_data_updated", &serde_json::json!({ "type": "contingent_updated" }));
                    }
                    (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response()
                },
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "delete_contingent" => {
            let id = get_i64_val(&payload, &["id"]).unwrap_or(0);
            match db.delete_contingent(id).await {
                Ok(()) => {
                    let _ = state.broadcast_tx.send(ArenaMessage {
                        tatami_pk: 0,
                        channel: "master-data".into(),
                        payload: serde_json::json!({ "type": "contingent_updated" }),
                    });
                    if let Some(app) = &state.app_handle {
                        let _ = app.emit("master_data_updated", &serde_json::json!({ "type": "contingent_updated" }));
                    }
                    (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response()
                },
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
                Ok(v) => {
                    let _ = state.broadcast_tx.send(ArenaMessage {
                        tatami_pk: 0,
                        channel: "master-data".into(),
                        payload: serde_json::json!({ "type": "perguruan_updated", "event_id": ev_id }),
                    });
                    if let Some(app) = &state.app_handle {
                        let _ = app.emit("master_data_updated", &serde_json::json!({ "type": "perguruan_updated", "event_id": ev_id }));
                    }
                    (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response()
                },
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "update_perguruan" => {
            let id = get_i64_val(&payload, &["id"]).unwrap_or(0);
            let nama = get_str_val(&payload, &["namaPerguruan", "nama_perguruan", "nama"]).unwrap_or_default();
            match db.update_perguruan(id, nama).await {
                Ok(v) => {
                    let _ = state.broadcast_tx.send(ArenaMessage {
                        tatami_pk: 0,
                        channel: "master-data".into(),
                        payload: serde_json::json!({ "type": "perguruan_updated" }),
                    });
                    if let Some(app) = &state.app_handle {
                        let _ = app.emit("master_data_updated", &serde_json::json!({ "type": "perguruan_updated" }));
                    }
                    (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response()
                },
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "delete_perguruan" => {
            let id = get_i64_val(&payload, &["id"]).unwrap_or(0);
            match db.delete_perguruan(id).await {
                Ok(()) => {
                    let _ = state.broadcast_tx.send(ArenaMessage {
                        tatami_pk: 0,
                        channel: "master-data".into(),
                        payload: serde_json::json!({ "type": "perguruan_updated" }),
                    });
                    if let Some(app) = &state.app_handle {
                        let _ = app.emit("master_data_updated", &serde_json::json!({ "type": "perguruan_updated" }));
                    }
                    (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response()
                },
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
        "get_festival_booklet_data" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            let tatami_id = get_i64_val(&payload, &["tatamiId", "tatami_id"]);
            match db.get_festival_booklet_data(ev_id, tatami_id).await {
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
        "get_team_match_info" => {
            let m_id = get_i64_val(&payload, &["matchId", "match_id"]).unwrap_or(0);
            match db.get_team_match_info(m_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "assign_bout_athletes" => {
            let bout_id = get_i64_val(&payload, &["boutId", "bout_id"]).unwrap_or(0);
            let aka_id = get_i64_val(&payload, &["akaAtletId", "aka_atlet_id"]);
            let ao_id = get_i64_val(&payload, &["aoAtletId", "ao_atlet_id"]);
            let aka_custom = get_str_val(&payload, &["akaCustomName", "aka_custom_name"]);
            let ao_custom = get_str_val(&payload, &["aoCustomName", "ao_custom_name"]);
            let input = AssignBoutAthletesInput {
                bout_id,
                aka_atlet_id: aka_id,
                ao_atlet_id: ao_id,
                aka_custom_name: aka_custom,
                ao_custom_name: ao_custom,
            };
            match db.assign_bout_athletes(input).await {
                Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "add_team_bout" => {
            let p_id = get_i64_val(&payload, &["parentMatchId", "parent_match_id"]).unwrap_or(0);
            match db.add_team_bout(p_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "delete_team_bout" => {
            let b_id = get_i64_val(&payload, &["boutId", "bout_id"]).unwrap_or(0);
            match db.delete_team_bout(b_id).await {
                Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "finish_team_match" => {
            let p_id = get_i64_val(&payload, &["parentMatchId", "parent_match_id"]).unwrap_or(0);
            let winner = get_str_val(&payload, &["winnerOverride", "winner_override"]);
            match db.finish_team_match(p_id, winner).await {
                Ok(_) => {
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
        "get_wasits" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_wasits(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "save_wasit" => {
            match serde_json::from_value::<SaveWasitInput>(payload.get("input").unwrap_or(&payload).clone()) {
                Ok(input) => match db.save_wasit(input).await {
                    Ok(id) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": id }))).into_response(),
                    Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
                },
                Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "status": "error", "message": format!("Invalid payload: {}", e) }))).into_response(),
            }
        },
        "delete_wasit" => {
            let wasit_id = get_i64_val(&payload, &["wasitId", "wasit_id", "id"]).unwrap_or(0);
            match db.delete_wasit(wasit_id).await {
                Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_tatami_panel_overview" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            let tatami_id = get_i64_val(&payload, &["tatamiId", "tatami_id"]).unwrap_or(1);
            match db.get_tatami_panel_overview(ev_id, tatami_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "assign_wasit_tatami" => {
            match serde_json::from_value::<AssignWasitTatamiInput>(payload.get("input").unwrap_or(&payload).clone()) {
                Ok(input) => match db.assign_wasit_tatami(input).await {
                    Ok(id) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": id }))).into_response(),
                    Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
                },
                Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "status": "error", "message": format!("Invalid payload: {}", e) }))).into_response(),
            }
        },
        "remove_wasit_tatami" => {
            let assign_id = get_i64_val(&payload, &["assignmentId", "assignment_id", "id"]).unwrap_or(0);
            match db.remove_wasit_tatami(assign_id).await {
                Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "clear_tatami_panel" => {
            let tatami_id = get_i64_val(&payload, &["tatamiId", "tatami_id"]).unwrap_or(0);
            match db.clear_tatami_panel(tatami_id).await {
                Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "auto_assign_tatami_panel" => {
            match serde_json::from_value::<AutoAssignPanelInput>(payload.get("input").unwrap_or(&payload).clone()) {
                Ok(input) => match db.auto_assign_tatami_panel(input).await {
                    Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
                    Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
                },
                Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "status": "error", "message": format!("Invalid payload: {}", e) }))).into_response(),
            }
        },
        "export_tournament_excel" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_tournament_excel_data(ev_id).await {
                Ok(data) => match crate::excel_exporter::generate_tournament_workbook(&data) {
                    Ok(bytes) => {
                        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
                        let clean_name: String = data.event_name.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
                        (StatusCode::OK, Json(serde_json::json!({
                            "status": "success",
                            "data": {
                                "base64": b64,
                                "filename": format!("Rekap_Kejuaraan_{}.xlsx", clean_name),
                                "size": bytes.len(),
                            }
                        }))).into_response()
                    },
                    Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": format!("Gagal generate excel: {}", e) }))).into_response(),
                },
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_marshalling_overview" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_marshalling_overview(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_results_booklet_data" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_results_booklet_data(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "get_bob_candidates" => {
            let ev_id = get_i64_val(&payload, &["eventId", "event_id"]).unwrap_or(1);
            match db.get_bob_candidates(ev_id).await {
                Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
            }
        },
        "create_bob_bagan" => {
            let input_val = payload.get("input").unwrap_or(&payload);
            match serde_json::from_value::<CreateBobBaganInput>(input_val.clone()) {
                Ok(input) => match db.create_bob_bagan(input).await {
                    Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
                    Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
                },
                Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "status": "error", "message": format!("Payload tidak valid: {}", e) }))).into_response(),
            }
        },
        "broadcast_urgent_call" => {
            match serde_json::from_value::<SubmitUrgentCallInput>(payload.get("input").unwrap_or(&payload).clone()) {
                Ok(input) => {
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                        .to_string();

                    let call_payload = serde_json::json!({
                        "type": "urgent_call",
                        "tatami_number": input.tatami_number,
                        "athlete_name": input.athlete_name,
                        "contingent": input.contingent.clone().unwrap_or_else(|| "-".to_string()),
                        "message": input.message,
                        "timestamp": ts,
                    });

                    let _ = state.broadcast_tx.send(ArenaMessage {
                        tatami_pk: input.tatami_number as i64,
                        channel: "marshalling".to_string(),
                        payload: call_payload.clone(),
                    });

                    if let Some(app) = &state.app_handle {
                        let _ = app.emit("marshalling_urgent_call", &call_payload);
                    }

                    (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response()
                },
                Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "status": "error", "message": format!("Invalid payload: {}", e) }))).into_response(),
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

async fn list_wasits_api(
    State(state): State<Arc<AppState>>,
    Path(event_id): Path<i64>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_wasits(event_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn save_wasit_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveWasitInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.save_wasit(payload).await {
        Ok(id) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": id }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

#[derive(Deserialize)]
struct WasitIdPayload {
    id: i64,
}

async fn delete_wasit_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<WasitIdPayload>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.delete_wasit(payload.id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn tatami_panel_overview_api(
    State(state): State<Arc<AppState>>,
    Path(tatami_id): Path<i64>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_tatami_panel_overview(1, tatami_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn assign_wasit_tatami_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AssignWasitTatamiInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.assign_wasit_tatami(payload).await {
        Ok(id) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": id }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn remove_wasit_tatami_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<WasitIdPayload>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.remove_wasit_tatami(payload.id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

#[derive(Deserialize)]
struct ClearPanelPayload {
    tatami_id: i64,
}

async fn clear_tatami_panel_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ClearPanelPayload>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.clear_tatami_panel(payload.tatami_id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn auto_assign_tatami_panel_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AutoAssignPanelInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.auto_assign_tatami_panel(payload).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}


#[derive(serde::Deserialize)]
pub struct ExportHtmlPdfInput {
    pub html_content: String,
    pub filename: Option<String>,
}

async fn export_html_to_pdf_api(
    axum::Json(payload): axum::Json<ExportHtmlPdfInput>,
) -> impl IntoResponse {
    let edge_paths = [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    ];
    let edge_exe = match edge_paths.iter().find(|p| std::path::Path::new(p).exists()) {
        Some(p) => *p,
        None => return (StatusCode::INTERNAL_SERVER_ERROR, "Microsoft Edge tidak ditemukan di server host.".to_string()).into_response(),
    };

    let temp_dir = std::env::temp_dir();
    let temp_id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
    let temp_html_path = temp_dir.join(format!("ambrilindo_api_booklet_{}.html", temp_id));
    let temp_pdf_path = temp_dir.join(format!("ambrilindo_api_booklet_{}.pdf", temp_id));

    if let Err(e) = std::fs::write(&temp_html_path, payload.html_content) {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write HTML: {}", e)).into_response();
    }

    let file_url = format!("file:///{}", temp_html_path.to_string_lossy().replace('\\', "/"));
    let res = tokio::process::Command::new(edge_exe)
        .arg("--headless")
        .arg("--disable-gpu")
        .arg("--no-pdf-header-footer")
        .arg(format!("--print-to-pdf={}", temp_pdf_path.to_string_lossy()))
        .arg(&file_url)
        .output()
        .await;

    let _ = std::fs::remove_file(&temp_html_path);

    match res {
        Ok(_) if temp_pdf_path.exists() => {
            let pdf_bytes = std::fs::read(&temp_pdf_path).unwrap_or_default();
            let _ = std::fs::remove_file(&temp_pdf_path);
            let fname = payload.filename.unwrap_or_else(|| "Buku_Hasil_Turnamen.pdf".to_string());
            (
                StatusCode::OK,
                [
                    (axum::http::header::CONTENT_TYPE, "application/pdf"),
                    (axum::http::header::CONTENT_DISPOSITION, &format!("attachment; filename=\"{}\"", fname)),
                ],
                pdf_bytes,
            ).into_response()
        },
        Ok(out) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Edge failed to generate PDF: {}", String::from_utf8_lossy(&out.stderr))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to spawn Edge: {}", e)).into_response(),
    }
}

async fn export_tournament_excel_api(
    State(state): State<Arc<AppState>>,
    Path(event_id): Path<i64>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_tournament_excel_data(event_id).await {
        Ok(data) => match crate::excel_exporter::generate_tournament_workbook(&data) {
            Ok(bytes) => {
                let clean_name: String = data
                    .event_name
                    .chars()
                    .map(|c| if c.is_alphanumeric() { c } else { '_' })
                    .collect();
                let filename = format!("Rekap_Kejuaraan_{}.xlsx", clean_name);
                (
                    StatusCode::OK,
                    [
                        (axum::http::header::CONTENT_TYPE, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
                        (axum::http::header::CONTENT_DISPOSITION, &format!("attachment; filename=\"{}\"", filename)),
                    ],
                    bytes,
                ).into_response()
            },
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to generate excel: {}", e)).into_response(),
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to load tournament data: {}", e)).into_response(),
    }
}

async fn get_marshalling_overview_api(
    State(state): State<Arc<AppState>>,
    Path(event_id): Path<i64>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_marshalling_overview(event_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn get_results_booklet_data_api(
    State(state): State<Arc<AppState>>,
    Path(event_id): Path<i64>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_results_booklet_data(event_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn get_bob_candidates_api(
    State(state): State<Arc<AppState>>,
    Path(event_id): Path<i64>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.get_bob_candidates(event_id).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn create_bob_bagan_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateBobBaganInput>,
) -> impl IntoResponse {
    let db = state.db.lock().await;
    match db.create_bob_bagan(payload).await {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!({ "status": "success", "data": v }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "status": "error", "message": e }))).into_response(),
    }
}

async fn submit_urgent_call_api(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SubmitUrgentCallInput>,
) -> impl IntoResponse {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();

    let call_payload = serde_json::json!({
        "type": "urgent_call",
        "tatami_number": payload.tatami_number,
        "athlete_name": payload.athlete_name,
        "contingent": payload.contingent.clone().unwrap_or_else(|| "-".to_string()),
        "message": payload.message,
        "timestamp": ts,
    });

    let _ = state.broadcast_tx.send(ArenaMessage {
        tatami_pk: payload.tatami_number as i64,
        channel: "marshalling".to_string(),
        payload: call_payload.clone(),
    });

    if let Some(app) = &state.app_handle {
        let _ = app.emit("marshalling_urgent_call", &call_payload);
    }

    (StatusCode::OK, Json(serde_json::json!({ "status": "success" }))).into_response()
}

pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let mut router = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/network-info", get(network_info_handler))
        .route("/api/network/ip-availability", get(ip_availability_api))
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
        .route("/api/bagan/repechage-info/:bagan_id", get(get_repechage_info_api))
        .route("/api/bagan/generate-repechage", post(generate_repechage_api))
        .route("/api/bagan/round-robin/:bagan_id", get(get_round_robin_data_api))
        .route("/api/bagan/create-round-robin", post(create_round_robin_bagan_api))
        .route("/api/detail-bagan/:bagan_id", get(detail_bagan_api))
        .route("/api/bagan/update-details", post(update_bagan_details_api))
        .route("/api/bagan/execute-drawing", post(execute_drawing_bagan_api))
        .route("/api/bagan/delete-category", post(delete_category_bagans_api))
        .route("/api/bagan/delete-all", post(delete_all_bagans_api))
        .route("/api/bagan/delete-all-festival", post(delete_all_festival_bagans_api))
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
        .route("/api/festival/drawing-eligible-categories/:event_id", get(drawing_eligible_festival_categories_api))
        .route("/api/festival/execute-drawing", post(execute_drawing_festival_bagan_api))
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
        .route("/api/festival-booklet/:event_id", get(get_festival_booklet_api))
        .route("/api/festival/save-state", post(save_festival_roster_api))
        .route("/api/festival/auto-pair", post(auto_pair_festival_api))
        .route("/api/festival/mix-categories", post(mix_festival_categories_api))
        // Video Review (VR) routes
        .route("/api/vr/challenge", post(submit_vr_challenge_api))
        .route("/api/vr/resolve", post(resolve_vr_challenge_api))
        // Wasit / Referee & Panel routes
        .route("/api/wasits/:event_id", get(list_wasits_api))
        .route("/api/wasit/save", post(save_wasit_api))
        .route("/api/wasit/delete", post(delete_wasit_api))
        .route("/api/tatami-panel/:tatami_id", get(tatami_panel_overview_api))
        .route("/api/tatami-panel/assign", post(assign_wasit_tatami_api))
        .route("/api/tatami-panel/remove", post(remove_wasit_tatami_api))
        .route("/api/tatami-panel/clear", post(clear_tatami_panel_api))
        .route("/api/tatami-panel/auto-assign", post(auto_assign_tatami_panel_api))
        // Official Multi-Sheet Excel Exporter
        .route("/api/export-excel/:event_id", get(export_tournament_excel_api))
        // A full results booklet with its cover and embedded fonts/logos is several MB;
        // the default 2 MB body limit made export from LAN devices fail.
        .route(
            "/api/export/html-to-pdf",
            post(export_html_to_pdf_api).layer(DefaultBodyLimit::max(64 * 1024 * 1024)),
        )
        // Athlete Marshalling & Call Room
        .route("/api/marshalling/:event_id", get(get_marshalling_overview_api))
        .route("/api/marshalling/urgent-call", post(submit_urgent_call_api))
        // Official Results Booklet & Minutes of Meeting (Berita Acara)
        .route("/api/results-booklet/:event_id", get(get_results_booklet_data_api))
        // Best of Best (BOB) Division Generator
        .route("/api/bob/candidates/:event_id", get(get_bob_candidates_api))
        .route("/api/bob/create", post(create_bob_bagan_api))
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
pub async fn start_server(state: Arc<AppState>) -> Result<SocketAddr, String> {
    // Hold the lock for the whole start so two concurrent starts cannot both bind.
    let mut running = RUNNING_SERVER.lock().await;
    if let Some(existing) = running.as_ref() {
        if !existing.handle.is_finished() {
            return Ok(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), DEFAULT_PORT));
        }
    }

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

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();

    let app = create_router(state);
    let handle = tokio::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = rx.await;
            info!("Arena Axum server received graceful shutdown signal");
        });
        if let Err(e) = server.await {
            error!("Server error: {}", e);
        }
    });

    *running = Some(RunningServer { shutdown_tx: tx, handle });
    Ok(local_addr)
}
