use axum::{Json, Router, extract::State};
use serde::Serialize;
use std::{net::SocketAddr, sync::Arc, time::Instant};
use tokio::{net::TcpListener, sync::RwLock};

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub peer_id: String,
    pub uptime_secs: u64,
    pub connections: usize,
}

pub struct HealthState {
    pub peer_id: String,
    pub start_time: Instant,
    pub connections: usize,
}

pub type SharedHealth = Arc<RwLock<HealthState>>;

async fn health(State(state): State<SharedHealth>) -> Json<HealthResponse> {
    let state = state.read().await;

    Json(HealthResponse {
        status: "ok".to_string(),
        peer_id: state.peer_id.clone(),
        uptime_secs: state.start_time.elapsed().as_secs(),
        connections: state.connections,
    })
}

pub async fn start_health_server(state: SharedHealth, addr: SocketAddr) {
    let app = Router::new()
        .route("/health", axum::routing::get(health))
        .with_state(state);

    axum::serve(TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap()
}
