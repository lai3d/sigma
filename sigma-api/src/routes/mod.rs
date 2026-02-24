pub mod agent;
pub mod ansible;
pub mod audit_logs;
pub mod auth_routes;
pub mod dns;
pub mod costs;
pub mod envoy;
pub mod exchange_rates;
pub mod ip_checks;
pub mod prometheus;
pub mod providers;
pub mod rate_limit;
pub mod stats;
pub mod tickets;
pub mod users;
pub mod vps;

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};

use crate::auth::{verify_token, CurrentUser};
use crate::db::Db;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub api_key: Option<String>,
    pub redis: redis::aio::ConnectionManager,
    pub rate_limit_requests: u32,
    pub rate_limit_window: u64,
    pub http_client: reqwest::Client,
    pub jwt_secret: String,
    pub jwt_expiry_hours: u64,
}

/// Auth middleware: try Bearer JWT → try X-Api-Key → allow if no API_KEY set → 401.
/// Inserts CurrentUser into request extensions on success.
pub async fn auth(
    state: axum::extract::State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // 1. Try Bearer JWT
    if let Some(auth_header) = req.headers().get("authorization").and_then(|v| v.to_str().ok()) {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            if let Ok(claims) = verify_token(token, &state.jwt_secret) {
                let user_id = claims.sub.parse::<uuid::Uuid>().map_err(|_| StatusCode::UNAUTHORIZED)?;
                req.extensions_mut().insert(CurrentUser {
                    id: user_id,
                    email: claims.email,
                    role: claims.role,
                    is_api_key: false,
                });
                return Ok(next.run(req).await);
            }
        }
    }

    // 2. Try X-Api-Key
    if let Some(ref expected) = state.api_key {
        let provided = req
            .headers()
            .get("x-api-key")
            .and_then(|v| v.to_str().ok());

        match provided {
            Some(key) if key == expected => {
                req.extensions_mut().insert(CurrentUser {
                    id: uuid::Uuid::nil(),
                    email: "api-key".to_string(),
                    role: "admin".to_string(),
                    is_api_key: true,
                });
                return Ok(next.run(req).await);
            }
            Some(_) => return Err(StatusCode::UNAUTHORIZED),
            None => {}
        }
    }

    // 3. If no API_KEY env is set, allow without auth (backwards compat)
    if state.api_key.is_none() {
        req.extensions_mut().insert(CurrentUser {
            id: uuid::Uuid::nil(),
            email: "anonymous".to_string(),
            role: "admin".to_string(),
            is_api_key: true,
        });
        return Ok(next.run(req).await);
    }

    Err(StatusCode::UNAUTHORIZED)
}
