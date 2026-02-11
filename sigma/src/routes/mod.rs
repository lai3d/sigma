pub mod prometheus;
pub mod providers;
pub mod stats;
pub mod vps;

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};

use crate::db::Db;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub api_key: Option<String>,
}

/// Simple API key auth middleware.
/// If API_KEY env is not set, all requests are allowed.
pub async fn auth(
    state: axum::extract::State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if let Some(ref expected) = state.api_key {
        let provided = req
            .headers()
            .get("x-api-key")
            .and_then(|v| v.to_str().ok());

        match provided {
            Some(key) if key == expected => {}
            _ => return Err(StatusCode::UNAUTHORIZED),
        }
    }
    Ok(next.run(req).await)
}
