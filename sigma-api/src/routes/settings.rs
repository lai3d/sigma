use axum::{extract::State, routing::get, Extension, Json, Router};
use std::collections::HashMap;

use crate::auth::{require_role, CurrentUser};
use crate::errors::{AppError, ErrorResponse};
use crate::models::SystemSetting;
use crate::routes::audit_logs::log_audit;
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/settings", get(list).put(update))
}

#[utoipa::path(
    get, path = "/api/settings",
    tag = "Settings",
    responses(
        (status = 200, description = "All settings as key-value object", body = HashMap<String, String>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list(
    State(state): State<AppState>,
) -> Result<Json<HashMap<String, String>>, AppError> {
    let rows = sqlx::query_as::<_, SystemSetting>("SELECT * FROM system_settings ORDER BY key")
        .fetch_all(&state.db)
        .await?;

    let map: HashMap<String, String> = rows.into_iter().map(|r| (r.key, r.value)).collect();
    Ok(Json(map))
}

#[utoipa::path(
    put, path = "/api/settings",
    tag = "Settings",
    request_body = HashMap<String, String>,
    responses(
        (status = 200, description = "Updated settings", body = HashMap<String, String>),
        (status = 403, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Json(input): Json<HashMap<String, String>>,
) -> Result<Json<HashMap<String, String>>, AppError> {
    require_role(&user, &["admin"])?;

    for (key, value) in &input {
        sqlx::query(
            r#"INSERT INTO system_settings (key, value, updated_at)
               VALUES ($1, $2, now())
               ON CONFLICT (key) DO UPDATE SET value = $2, updated_at = now()"#,
        )
        .bind(key)
        .bind(value)
        .execute(&state.db)
        .await?;
    }

    log_audit(
        &state.db,
        &user,
        "update",
        "system_settings",
        None,
        serde_json::json!(input),
    )
    .await;

    // Return current state
    let rows = sqlx::query_as::<_, SystemSetting>("SELECT * FROM system_settings ORDER BY key")
        .fetch_all(&state.db)
        .await?;

    let map: HashMap<String, String> = rows.into_iter().map(|r| (r.key, r.value)).collect();
    Ok(Json(map))
}
