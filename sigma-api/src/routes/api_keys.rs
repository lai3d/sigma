use axum::{
    extract::{Path, Query, State},
    routing::get,
    Extension, Json, Router,
};
use uuid::Uuid;

use crate::auth::{generate_api_key, require_role, CurrentUser};
use crate::errors::{AppError, ErrorResponse};
#[allow(unused_imports)]
use crate::models::PaginatedApiKeyResponse;
use crate::models::{ApiKey, ApiKeyCreatedResponse, ApiKeyListQuery, ApiKeyResponse, CreateApiKey, PaginatedResponse};
use crate::routes::audit_logs::log_audit;
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/api-keys", get(list).post(create))
        .route("/api/api-keys/{id}", get(get_one).delete(delete))
}

fn validate_role(role: &str) -> Result<(), AppError> {
    match role {
        "admin" | "operator" | "readonly" | "agent" => Ok(()),
        _ => Err(AppError::BadRequest(
            "role must be one of: admin, operator, readonly, agent".into(),
        )),
    }
}

#[utoipa::path(
    get, path = "/api/api-keys",
    tag = "API Keys",
    params(ApiKeyListQuery),
    responses(
        (status = 200, body = PaginatedApiKeyResponse),
        (status = 403, body = ErrorResponse),
    ),
    security(("bearer" = []))
)]
pub async fn list(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Query(q): Query<ApiKeyListQuery>,
) -> Result<Json<PaginatedResponse<ApiKeyResponse>>, AppError> {
    require_role(&current, &["admin"])?;

    let per_page = q.per_page.clamp(1, 100);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM api_keys")
        .fetch_one(&state.db)
        .await?;

    let rows = sqlx::query_as::<_, ApiKey>(
        "SELECT * FROM api_keys ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(PaginatedResponse {
        data: rows.into_iter().map(ApiKeyResponse::from).collect(),
        total: total.0,
        page,
        per_page,
    }))
}

#[utoipa::path(
    get, path = "/api/api-keys/{id}",
    tag = "API Keys",
    params(("id" = Uuid, Path, description = "API Key ID")),
    responses(
        (status = 200, body = ApiKeyResponse),
        (status = 404, body = ErrorResponse),
    ),
    security(("bearer" = []))
)]
pub async fn get_one(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiKeyResponse>, AppError> {
    require_role(&current, &["admin"])?;

    let row = sqlx::query_as::<_, ApiKey>("SELECT * FROM api_keys WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(row.into()))
}

#[utoipa::path(
    post, path = "/api/api-keys",
    tag = "API Keys",
    request_body = CreateApiKey,
    responses(
        (status = 200, body = ApiKeyCreatedResponse),
        (status = 400, body = ErrorResponse),
        (status = 403, body = ErrorResponse),
    ),
    security(("bearer" = []))
)]
pub async fn create(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Json(input): Json<CreateApiKey>,
) -> Result<Json<ApiKeyCreatedResponse>, AppError> {
    require_role(&current, &["admin"])?;

    validate_role(&input.role)?;

    if input.name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".into()));
    }

    let (plaintext, key_hash, key_prefix) = generate_api_key();

    let created_by = if current.id.is_nil() { None } else { Some(current.id) };

    let row = sqlx::query_as::<_, ApiKey>(
        "INSERT INTO api_keys (name, key_hash, key_prefix, role, created_by) VALUES ($1, $2, $3, $4, $5) RETURNING *",
    )
    .bind(input.name.trim())
    .bind(&key_hash)
    .bind(&key_prefix)
    .bind(&input.role)
    .bind(created_by)
    .fetch_one(&state.db)
    .await?;

    log_audit(&state.db, &current, "create", "api_key", Some(&row.id.to_string()),
        serde_json::json!({"name": row.name, "role": row.role, "key_prefix": row.key_prefix})).await;

    Ok(Json(ApiKeyCreatedResponse {
        id: row.id,
        name: row.name,
        key: plaintext,
        key_prefix: row.key_prefix,
        role: row.role,
        created_at: row.created_at,
    }))
}

#[utoipa::path(
    delete, path = "/api/api-keys/{id}",
    tag = "API Keys",
    params(("id" = Uuid, Path, description = "API Key ID")),
    responses(
        (status = 204),
        (status = 404, body = ErrorResponse),
        (status = 403, body = ErrorResponse),
    ),
    security(("bearer" = []))
)]
pub async fn delete(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    require_role(&current, &["admin"])?;

    let row = sqlx::query_as::<_, ApiKey>("DELETE FROM api_keys WHERE id = $1 RETURNING *")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    log_audit(&state.db, &current, "delete", "api_key", Some(&id.to_string()),
        serde_json::json!({"name": row.name, "role": row.role, "key_prefix": row.key_prefix})).await;

    Ok(StatusCode::NO_CONTENT)
}

use axum::http::StatusCode;
