pub mod alibaba;
pub mod aws;
pub mod digitalocean;
pub mod linode;
pub mod volcengine;

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Extension, Json, Router,
};
use uuid::Uuid;

use crate::auth::{require_role, CurrentUser};
use crate::errors::AppError;
use crate::models::{
    CloudAccount, CloudAccountListQuery, CloudAccountResponse, CloudSyncResult,
    CreateCloudAccount, PaginatedCloudAccountResponse, PaginatedResponse, UpdateCloudAccount,
};
use crate::routes::audit_logs::log_audit;
use crate::routes::AppState;

const VALID_PROVIDER_TYPES: &[&str] = &["aws", "alibaba", "digitalocean", "linode", "volcengine"];

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/cloud-accounts",
            get(list_accounts).post(create_account),
        )
        .route(
            "/api/cloud-accounts/{id}",
            get(get_account).put(update_account).delete(delete_account),
        )
        .route(
            "/api/cloud-accounts/{id}/sync",
            axum::routing::post(sync_account),
        )
}

// ─── Provider dispatch helpers ───────────────────────────

async fn validate_credentials(
    state: &AppState,
    provider_type: &str,
    config: &serde_json::Value,
) -> Result<(), AppError> {
    match provider_type {
        "aws" => aws::validate(config).await,
        "alibaba" => alibaba::validate(config).await,
        "digitalocean" => digitalocean::validate(&state.http_client, config).await,
        "linode" => linode::validate(&state.http_client, config).await,
        "volcengine" => volcengine::validate(config).await,
        _ => Err(AppError::BadRequest(format!(
            "Unknown provider type: {provider_type}"
        ))),
    }
}

fn mask_config(provider_type: &str, config: &serde_json::Value) -> serde_json::Value {
    match provider_type {
        "aws" => aws::mask_config(config),
        "alibaba" => alibaba::mask_config(config),
        "digitalocean" => digitalocean::mask_config(config),
        "linode" => linode::mask_config(config),
        "volcengine" => volcengine::mask_config(config),
        _ => serde_json::json!({}),
    }
}

async fn sync_provider(
    state: &AppState,
    account: &CloudAccount,
) -> Result<CloudSyncResult, AppError> {
    match account.provider_type.as_str() {
        "aws" => aws::sync(state, account).await,
        "alibaba" => alibaba::sync(state, account).await,
        "digitalocean" => digitalocean::sync(state, account).await,
        "linode" => linode::sync(state, account).await,
        "volcengine" => volcengine::sync(state, account).await,
        _ => Err(AppError::BadRequest(format!(
            "Unknown provider type: {}",
            account.provider_type
        ))),
    }
}

/// Build a CloudAccountResponse from a CloudAccount by querying VPS count.
async fn build_account_response(
    state: &AppState,
    acc: &CloudAccount,
) -> Result<CloudAccountResponse, AppError> {
    let (vps_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM vps WHERE cloud_account_id = $1")
            .bind(acc.id)
            .fetch_one(&state.db)
            .await?;

    Ok(CloudAccountResponse {
        id: acc.id,
        name: acc.name.clone(),
        provider_type: acc.provider_type.clone(),
        masked_config: mask_config(&acc.provider_type, &acc.config),
        vps_count,
        last_synced_at: acc.last_synced_at,
        created_at: acc.created_at,
        updated_at: acc.updated_at,
    })
}

// ─── Account CRUD ─────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/cloud-accounts",
    tag = "Cloud Accounts",
    params(CloudAccountListQuery),
    responses((status = 200, body = PaginatedCloudAccountResponse))
)]
pub async fn list_accounts(
    State(state): State<AppState>,
    Query(q): Query<CloudAccountListQuery>,
) -> Result<Json<PaginatedResponse<CloudAccountResponse>>, AppError> {
    let per_page = q.per_page.clamp(1, 100);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cloud_accounts")
        .fetch_one(&state.db)
        .await?;

    let rows = sqlx::query_as::<_, CloudAccount>(
        "SELECT * FROM cloud_accounts ORDER BY name LIMIT $1 OFFSET $2",
    )
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    let mut data = Vec::with_capacity(rows.len());
    for acc in &rows {
        data.push(build_account_response(&state, acc).await?);
    }

    Ok(Json(PaginatedResponse {
        data,
        total: total.0,
        page,
        per_page,
    }))
}

#[utoipa::path(
    get,
    path = "/api/cloud-accounts/{id}",
    tag = "Cloud Accounts",
    params(("id" = Uuid, Path, description = "Account ID")),
    responses(
        (status = 200, body = CloudAccountResponse),
        (status = 404),
    )
)]
pub async fn get_account(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CloudAccountResponse>, AppError> {
    let acc = sqlx::query_as::<_, CloudAccount>(
        "SELECT * FROM cloud_accounts WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(build_account_response(&state, &acc).await?))
}

#[utoipa::path(
    post,
    path = "/api/cloud-accounts",
    tag = "Cloud Accounts",
    request_body = CreateCloudAccount,
    responses(
        (status = 200, body = CloudAccountResponse),
        (status = 400),
    )
)]
pub async fn create_account(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Json(input): Json<CreateCloudAccount>,
) -> Result<Json<CloudAccountResponse>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    if !VALID_PROVIDER_TYPES.contains(&input.provider_type.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Invalid provider_type: {}. Must be one of: {}",
            input.provider_type,
            VALID_PROVIDER_TYPES.join(", ")
        )));
    }

    validate_credentials(&state, &input.provider_type, &input.config).await?;

    let acc = sqlx::query_as::<_, CloudAccount>(
        "INSERT INTO cloud_accounts (name, provider_type, config) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(&input.name)
    .bind(&input.provider_type)
    .bind(&input.config)
    .fetch_one(&state.db)
    .await?;

    log_audit(
        &state.db,
        &user,
        "create",
        "cloud_account",
        Some(&acc.id.to_string()),
        serde_json::json!({"name": acc.name, "provider_type": acc.provider_type}),
    )
    .await;

    Ok(Json(CloudAccountResponse {
        id: acc.id,
        name: acc.name.clone(),
        provider_type: acc.provider_type.clone(),
        masked_config: mask_config(&acc.provider_type, &acc.config),
        vps_count: 0,
        last_synced_at: None,
        created_at: acc.created_at,
        updated_at: acc.updated_at,
    }))
}

#[utoipa::path(
    put,
    path = "/api/cloud-accounts/{id}",
    tag = "Cloud Accounts",
    params(("id" = Uuid, Path, description = "Account ID")),
    request_body = UpdateCloudAccount,
    responses(
        (status = 200, body = CloudAccountResponse),
        (status = 404),
    )
)]
pub async fn update_account(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateCloudAccount>,
) -> Result<Json<CloudAccountResponse>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    let existing = sqlx::query_as::<_, CloudAccount>(
        "SELECT * FROM cloud_accounts WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let new_name = input.name.unwrap_or(existing.name.clone());
    let new_config = input.config.unwrap_or(existing.config.clone());

    // Re-validate if config changed
    if new_config != existing.config {
        validate_credentials(&state, &existing.provider_type, &new_config).await?;
    }

    let acc = sqlx::query_as::<_, CloudAccount>(
        "UPDATE cloud_accounts SET name = $2, config = $3, updated_at = now() WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(&new_name)
    .bind(&new_config)
    .fetch_one(&state.db)
    .await?;

    log_audit(
        &state.db,
        &user,
        "update",
        "cloud_account",
        Some(&id.to_string()),
        serde_json::json!({"name": acc.name}),
    )
    .await;

    Ok(Json(build_account_response(&state, &acc).await?))
}

#[utoipa::path(
    delete,
    path = "/api/cloud-accounts/{id}",
    tag = "Cloud Accounts",
    params(("id" = Uuid, Path, description = "Account ID")),
    responses((status = 200), (status = 404))
)]
pub async fn delete_account(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    // Unlink VPS records (ON DELETE SET NULL handles cloud_account_id)
    let result = sqlx::query("DELETE FROM cloud_accounts WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    log_audit(
        &state.db,
        &user,
        "delete",
        "cloud_account",
        Some(&id.to_string()),
        serde_json::json!({}),
    )
    .await;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

// ─── Sync ─────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/api/cloud-accounts/{id}/sync",
    tag = "Cloud Accounts",
    params(("id" = Uuid, Path, description = "Account ID")),
    responses(
        (status = 200, body = CloudSyncResult),
        (status = 404),
    )
)]
pub async fn sync_account(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<CloudSyncResult>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    let acc = sqlx::query_as::<_, CloudAccount>(
        "SELECT * FROM cloud_accounts WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let result = sync_provider(&state, &acc).await?;

    // Update last_synced_at
    sqlx::query("UPDATE cloud_accounts SET last_synced_at = now() WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    log_audit(
        &state.db,
        &user,
        "sync",
        "cloud_account",
        Some(&id.to_string()),
        serde_json::json!({
            "provider_type": acc.provider_type,
            "instances_found": result.instances_found,
            "created": result.created,
            "updated": result.updated,
            "retired": result.retired,
        }),
    )
    .await;

    Ok(Json(result))
}
