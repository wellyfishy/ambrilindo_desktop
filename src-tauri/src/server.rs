use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use qrcode::render::svg;
use qrcode::QrCode;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{error, info};

use axum::http::StatusCode;
use tokio::sync::Mutex;

pub const DEFAULT_PORT: u16 = 8000;

#[derive(Clone)]
pub struct AppState {
    pub broadcast_tx: broadcast::Sender<ArenaMessage>,
    pub db: Arc<Mutex<crate::db::DatabaseManager>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArenaMessage {
    pub tatami_pk: i64,
    pub channel: String, // "scoring-board", "jury-panel", "admin-control"
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub port: u16,
    pub local_ip: String,
    pub server_url: String,
    pub qr_svg: String,
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

pub fn get_network_info() -> NetworkInfo {
    let ip = get_local_ip();
    let url = format!("http://{}:{}", ip, DEFAULT_PORT);
    let qr_svg = generate_qr_svg(&url);
    NetworkInfo {
        port: DEFAULT_PORT,
        local_ip: ip,
        server_url: url,
        qr_svg,
    }
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
                            let _ = state.broadcast_tx.send(arena_msg);
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

pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/health", get(health_check))
        .route("/api/network-info", get(network_info_handler))
        .route("/api/events", get(list_events_api))
        .route("/api/tatamis/:event_id", get(list_tatamis_api))
        .route("/api/bagans/:event_id", get(list_bagans_api))
        .route("/api/detail-bagan/:bagan_id", get(detail_bagan_api))
        .route("/api/athletes/:event_id", get(list_athletes_api))
        .route("/api/contingents/:event_id", get(list_contingents_api))
        .route("/ws/:channel/:tatami_pk", get(ws_handler))
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
