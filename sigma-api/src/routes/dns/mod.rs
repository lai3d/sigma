pub mod cloudflare;
pub mod godaddy;
pub mod namecom;
pub mod route53;

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Extension, Json, Router,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::auth::{require_role, CurrentUser};
use crate::errors::AppError;
use crate::models::{
    CreateDnsAccount, DnsAccount, DnsAccountListQuery, DnsAccountResponse, DnsRecord,
    DnsRecordListQuery, DnsSyncResult, DnsZone, DnsZoneListQuery, PaginatedDnsAccountResponse,
    PaginatedDnsRecordResponse, PaginatedDnsZoneResponse, PaginatedResponse, UpdateDnsAccount,
};
use crate::routes::audit_logs::log_audit;
use crate::routes::AppState;

const VALID_PROVIDER_TYPES: &[&str] = &["cloudflare", "route53", "godaddy", "namecom"];

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/dns-accounts",
            get(list_accounts).post(create_account),
        )
        .route(
            "/api/dns-accounts/{id}",
            get(get_account).put(update_account).delete(delete_account),
        )
        .route(
            "/api/dns-accounts/{id}/sync",
            axum::routing::post(sync_account),
        )
        .route("/api/dns-zones", get(list_zones))
        .route("/api/dns-records", get(list_dns_records))
}

// ─── Provider dispatch helpers ───────────────────────────

async fn validate_credentials(
    http_client: &reqwest::Client,
    provider_type: &str,
    config: &serde_json::Value,
) -> Result<(), AppError> {
    match provider_type {
        "cloudflare" => cloudflare::validate(http_client, config).await,
        "route53" => route53::validate(config).await,
        "godaddy" => godaddy::validate(http_client, config).await,
        "namecom" => namecom::validate(http_client, config).await,
        _ => Err(AppError::BadRequest(format!(
            "Unknown provider type: {provider_type}"
        ))),
    }
}

fn mask_config(provider_type: &str, config: &serde_json::Value) -> serde_json::Value {
    match provider_type {
        "cloudflare" => cloudflare::mask_config(config),
        "route53" => route53::mask_config(config),
        "godaddy" => godaddy::mask_config(config),
        "namecom" => namecom::mask_config(config),
        _ => serde_json::json!({}),
    }
}

async fn sync_provider(
    state: &AppState,
    account: &DnsAccount,
) -> Result<DnsSyncResult, AppError> {
    match account.provider_type.as_str() {
        "cloudflare" => cloudflare::sync(state, account).await,
        "route53" => route53::sync(state, account).await,
        "godaddy" => godaddy::sync(state, account).await,
        "namecom" => namecom::sync(state, account).await,
        _ => Err(AppError::BadRequest(format!(
            "Unknown provider type: {}",
            account.provider_type
        ))),
    }
}

/// Build a DnsAccountResponse from a DnsAccount by querying zone/record counts.
async fn build_account_response(
    state: &AppState,
    acc: &DnsAccount,
) -> Result<DnsAccountResponse, AppError> {
    let (zones_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM dns_zones WHERE account_id = $1")
            .bind(acc.id)
            .fetch_one(&state.db)
            .await?;
    let (records_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM dns_records r JOIN dns_zones z ON z.id = r.zone_uuid WHERE z.account_id = $1",
    )
    .bind(acc.id)
    .fetch_one(&state.db)
    .await?;
    let last_synced: (Option<DateTime<Utc>>,) = sqlx::query_as(
        "SELECT MAX(synced_at) FROM dns_zones WHERE account_id = $1",
    )
    .bind(acc.id)
    .fetch_one(&state.db)
    .await?;

    Ok(DnsAccountResponse {
        id: acc.id,
        name: acc.name.clone(),
        provider_type: acc.provider_type.clone(),
        masked_config: mask_config(&acc.provider_type, &acc.config),
        zones_count,
        records_count,
        last_synced: last_synced.0,
        created_at: acc.created_at,
        updated_at: acc.updated_at,
    })
}

// ─── Account CRUD ─────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/dns-accounts",
    tag = "DNS",
    params(DnsAccountListQuery),
    responses((status = 200, body = PaginatedDnsAccountResponse))
)]
pub async fn list_accounts(
    State(state): State<AppState>,
    Query(q): Query<DnsAccountListQuery>,
) -> Result<Json<PaginatedResponse<DnsAccountResponse>>, AppError> {
    let per_page = q.per_page.clamp(1, 100);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM dns_accounts")
        .fetch_one(&state.db)
        .await?;

    let rows = sqlx::query_as::<_, DnsAccount>(
        "SELECT * FROM dns_accounts ORDER BY name LIMIT $1 OFFSET $2",
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
    path = "/api/dns-accounts/{id}",
    tag = "DNS",
    params(("id" = Uuid, Path, description = "Account ID")),
    responses(
        (status = 200, body = DnsAccountResponse),
        (status = 404),
    )
)]
pub async fn get_account(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<DnsAccountResponse>, AppError> {
    let acc = sqlx::query_as::<_, DnsAccount>(
        "SELECT * FROM dns_accounts WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(build_account_response(&state, &acc).await?))
}

#[utoipa::path(
    post,
    path = "/api/dns-accounts",
    tag = "DNS",
    request_body = CreateDnsAccount,
    responses(
        (status = 200, body = DnsAccountResponse),
        (status = 400),
    )
)]
pub async fn create_account(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Json(input): Json<CreateDnsAccount>,
) -> Result<Json<DnsAccountResponse>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    if !VALID_PROVIDER_TYPES.contains(&input.provider_type.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Invalid provider_type: {}. Must be one of: {}",
            input.provider_type,
            VALID_PROVIDER_TYPES.join(", ")
        )));
    }

    // Validate credentials with the provider
    validate_credentials(&state.http_client, &input.provider_type, &input.config).await?;

    let acc = sqlx::query_as::<_, DnsAccount>(
        "INSERT INTO dns_accounts (name, provider_type, config) VALUES ($1, $2, $3) RETURNING *",
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
        "dns_account",
        Some(&acc.id.to_string()),
        serde_json::json!({"name": acc.name, "provider_type": acc.provider_type}),
    )
    .await;

    Ok(Json(DnsAccountResponse {
        id: acc.id,
        name: acc.name.clone(),
        provider_type: acc.provider_type.clone(),
        masked_config: mask_config(&acc.provider_type, &acc.config),
        zones_count: 0,
        records_count: 0,
        last_synced: None,
        created_at: acc.created_at,
        updated_at: acc.updated_at,
    }))
}

#[utoipa::path(
    put,
    path = "/api/dns-accounts/{id}",
    tag = "DNS",
    params(("id" = Uuid, Path, description = "Account ID")),
    request_body = UpdateDnsAccount,
    responses(
        (status = 200, body = DnsAccountResponse),
        (status = 404),
    )
)]
pub async fn update_account(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateDnsAccount>,
) -> Result<Json<DnsAccountResponse>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    let existing = sqlx::query_as::<_, DnsAccount>(
        "SELECT * FROM dns_accounts WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let new_name = input.name.unwrap_or(existing.name.clone());
    let new_config = input.config.unwrap_or(existing.config.clone());

    let acc = sqlx::query_as::<_, DnsAccount>(
        "UPDATE dns_accounts SET name = $2, config = $3, updated_at = now() WHERE id = $1 RETURNING *",
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
        "dns_account",
        Some(&id.to_string()),
        serde_json::json!({"name": acc.name}),
    )
    .await;

    Ok(Json(build_account_response(&state, &acc).await?))
}

#[utoipa::path(
    delete,
    path = "/api/dns-accounts/{id}",
    tag = "DNS",
    params(("id" = Uuid, Path, description = "Account ID")),
    responses((status = 200), (status = 404))
)]
pub async fn delete_account(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    let result = sqlx::query("DELETE FROM dns_accounts WHERE id = $1")
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
        "dns_account",
        Some(&id.to_string()),
        serde_json::json!({}),
    )
    .await;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

// ─── Sync ─────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/api/dns-accounts/{id}/sync",
    tag = "DNS",
    params(("id" = Uuid, Path, description = "Account ID")),
    responses(
        (status = 200, body = DnsSyncResult),
        (status = 404),
    )
)]
pub async fn sync_account(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<DnsSyncResult>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    let acc = sqlx::query_as::<_, DnsAccount>(
        "SELECT * FROM dns_accounts WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let result = sync_provider(&state, &acc).await?;

    log_audit(
        &state.db,
        &user,
        "sync",
        "dns_account",
        Some(&id.to_string()),
        serde_json::json!({
            "provider_type": acc.provider_type,
            "zones": result.zones_count,
            "records": result.records_count,
            "linked": result.records_linked,
        }),
    )
    .await;

    Ok(Json(result))
}

// ─── Zones ────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/dns-zones",
    tag = "DNS",
    params(DnsZoneListQuery),
    responses((status = 200, body = PaginatedDnsZoneResponse))
)]
pub async fn list_zones(
    State(state): State<AppState>,
    Query(q): Query<DnsZoneListQuery>,
) -> Result<Json<PaginatedResponse<DnsZone>>, AppError> {
    let per_page = q.per_page.clamp(1, 100);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let mut where_clause = String::from(" WHERE 1=1");
    let mut param_idx = 0u32;

    if q.account_id.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND account_id = ${param_idx}"));
    }

    let count_sql = format!("SELECT COUNT(*) FROM dns_zones{where_clause}");
    let mut count_query = sqlx::query_as::<_, (i64,)>(&count_sql);
    if let Some(ref v) = q.account_id {
        count_query = count_query.bind(v);
    }
    let total = count_query.fetch_one(&state.db).await?.0;

    param_idx += 1;
    let limit_param = param_idx;
    param_idx += 1;
    let offset_param = param_idx;

    let data_sql = format!(
        "SELECT * FROM dns_zones{where_clause} ORDER BY zone_name LIMIT ${limit_param} OFFSET ${offset_param}"
    );
    let mut query = sqlx::query_as::<_, DnsZone>(&data_sql);
    if let Some(ref v) = q.account_id {
        query = query.bind(v);
    }
    query = query.bind(per_page).bind(offset);

    let rows = query.fetch_all(&state.db).await?;

    Ok(Json(PaginatedResponse {
        data: rows,
        total,
        page,
        per_page,
    }))
}

// ─── DNS Records ──────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/dns-records",
    tag = "DNS",
    params(DnsRecordListQuery),
    responses((status = 200, body = PaginatedDnsRecordResponse))
)]
pub async fn list_dns_records(
    State(state): State<AppState>,
    Query(q): Query<DnsRecordListQuery>,
) -> Result<Json<PaginatedResponse<DnsRecord>>, AppError> {
    let per_page = q.per_page.clamp(1, 100);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let mut where_clause = String::from(" WHERE 1=1");
    let mut param_idx = 0u32;

    if q.account_id.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND z.account_id = ${param_idx}"));
    }
    if q.zone_name.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND z.zone_name = ${param_idx}"));
    }
    if q.record_type.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND d.record_type = ${param_idx}"));
    }
    if let Some(has_vps) = q.has_vps {
        if has_vps {
            where_clause.push_str(" AND d.vps_id IS NOT NULL");
        } else {
            where_clause.push_str(" AND d.vps_id IS NULL");
        }
    }

    let count_sql = format!(
        "SELECT COUNT(*) FROM dns_records d JOIN dns_zones z ON z.id = d.zone_uuid{where_clause}"
    );
    let mut count_query = sqlx::query_as::<_, (i64,)>(&count_sql);
    if let Some(ref v) = q.account_id {
        count_query = count_query.bind(v);
    }
    if let Some(ref v) = q.zone_name {
        count_query = count_query.bind(v);
    }
    if let Some(ref v) = q.record_type {
        count_query = count_query.bind(v);
    }
    let total = count_query.fetch_one(&state.db).await?.0;

    param_idx += 1;
    let limit_param = param_idx;
    param_idx += 1;
    let offset_param = param_idx;

    let data_sql = format!(
        r#"SELECT d.id, d.zone_uuid, d.record_id, d.record_type, d.name, d.content,
                  d.ttl, d.extra, d.vps_id, d.synced_at, d.created_at, d.updated_at,
                  z.zone_name, z.zone_id AS zone_id_ext,
                  v.hostname AS vps_hostname, v.country AS vps_country
           FROM dns_records d
           JOIN dns_zones z ON z.id = d.zone_uuid
           LEFT JOIN vps v ON v.id = d.vps_id
           {where_clause}
           ORDER BY z.zone_name, d.name
           LIMIT ${limit_param} OFFSET ${offset_param}"#
    );

    let mut query = sqlx::query_as::<_, DnsRecord>(&data_sql);
    if let Some(ref v) = q.account_id {
        query = query.bind(v);
    }
    if let Some(ref v) = q.zone_name {
        query = query.bind(v);
    }
    if let Some(ref v) = q.record_type {
        query = query.bind(v);
    }
    query = query.bind(per_page).bind(offset);

    let rows = query.fetch_all(&state.db).await?;

    Ok(Json(PaginatedResponse {
        data: rows,
        total,
        page,
        per_page,
    }))
}
