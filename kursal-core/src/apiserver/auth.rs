use crate::apiserver::{APIAppState, types::APIError};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::{net::SocketAddr, time::Instant};

pub async fn auth_middleware(
    State(state): State<APIAppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, APIError> {
    let ip = addr.ip();
    let now = Instant::now();

    {
        let mut map = state.rate_map.lock().await;
        map.retain(|_, record| !record.is_stale(now));
        if let Some(record) = map.get_mut(&ip)
            && record.is_limited(now)
        {
            return Err(APIError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "Too many failed attempts",
            ));
        }
    }

    let token_match = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|v| {
            if let Ok(auth_token) = PasswordHash::new(&state.auth_token) {
                Argon2::default()
                    .verify_password(v.as_bytes(), &auth_token)
                    .is_ok()
            } else {
                false
            }
        });

    if token_match {
        state.rate_map.lock().await.remove(&ip);
        Ok(next.run(req).await)
    } else {
        state
            .rate_map
            .lock()
            .await
            .entry(ip)
            .or_default()
            .record_failure(now);

        Err(APIError::new(
            StatusCode::UNAUTHORIZED,
            "Missing or invalid bearer token",
        ))
    }
}
