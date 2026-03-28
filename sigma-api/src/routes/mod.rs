pub mod agent;
pub mod ansible;
pub mod api_keys;
pub mod audit_logs;
pub mod auth_routes;
pub mod cloud;
pub mod dns;
pub mod costs;
pub mod envoy;
pub mod exchange_rates;
pub mod ip_checks;
pub mod ip_labels;
pub mod prometheus;
pub mod providers;
pub mod rate_limit;
pub mod settings;
pub mod stats;
pub mod tickets;
pub mod users;
pub mod vps;
pub mod vps_duplicates;
pub mod vps_purposes;

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};

use crate::auth::{hash_api_key, verify_token, CurrentUser};
use crate::db::Db;
use crate::models::ApiKey;

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
    if let Some(provided_key) = req.headers().get("x-api-key").and_then(|v| v.to_str().ok()) {
        let key_hash = hash_api_key(provided_key);

        // 2a. Check database for managed API keys
        if let Ok(Some(api_key_row)) = sqlx::query_as::<_, ApiKey>(
            "SELECT * FROM api_keys WHERE key_hash = $1",
        )
        .bind(&key_hash)
        .fetch_optional(&state.db)
        .await
        {
            // Fire-and-forget: update last_used_at
            let db = state.db.clone();
            let key_id = api_key_row.id;
            tokio::spawn(async move {
                let _ = sqlx::query("UPDATE api_keys SET last_used_at = now() WHERE id = $1")
                    .bind(key_id)
                    .execute(&db)
                    .await;
            });

            req.extensions_mut().insert(CurrentUser {
                id: api_key_row.id,
                email: format!("api-key:{}", api_key_row.name),
                role: api_key_row.role,
                is_api_key: true,
            });
            return Ok(next.run(req).await);
        }

        // 2b. Legacy: check static API_KEY env var (backwards compat)
        if let Some(ref expected) = state.api_key {
            if provided_key == expected {
                req.extensions_mut().insert(CurrentUser {
                    id: uuid::Uuid::nil(),
                    email: "api-key:legacy".to_string(),
                    role: "admin".to_string(),
                    is_api_key: true,
                });
                return Ok(next.run(req).await);
            }
        }

        // Key was provided but matched nothing
        return Err(StatusCode::UNAUTHORIZED);
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
