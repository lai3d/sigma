use axum::{
    extract::{Path, Query, State},
    routing::get,
    Extension, Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::{require_role, CurrentUser};
use crate::errors::AppError;
use crate::models::{
    CloudflareAccount, CloudflareAccountListQuery, CloudflareAccountResponse,
    CloudflareDnsListQuery, CloudflareDnsRecord, CloudflareSyncResult, CloudflareZone,
    CloudflareZoneListQuery, CreateCloudflareAccount, PaginatedCloudflareAccountResponse,
    PaginatedCloudflareDnsResponse, PaginatedCloudflareZoneResponse, PaginatedResponse,
    UpdateCloudflareAccount,
};
use crate::routes::audit_logs::log_audit;
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/cloudflare-accounts",
            get(list_accounts).post(create_account),
        )
        .route(
            "/api/cloudflare-accounts/{id}",
            get(get_account).put(update_account).delete(delete_account),
        )
        .route(
            "/api/cloudflare-accounts/{id}/sync",
            axum::routing::post(sync_account),
        )
        .route("/api/cloudflare-zones", get(list_zones))
        .route("/api/cloudflare-dns-records", get(list_dns_records))
}

// ─── Cloudflare API response structs ──────────────────────

#[derive(Deserialize)]
struct CfResponse<T> {
    result: T,
    result_info: Option<CfResultInfo>,
    success: bool,
}

#[derive(Deserialize)]
struct CfResultInfo {
    #[allow(dead_code)]
    page: u32,
    total_pages: u32,
}

#[derive(Deserialize)]
struct CfZone {
    id: String,
    name: String,
    status: String,
    account: CfAccountRef,
}

#[derive(Deserialize)]
struct CfAccountRef {
    id: String,
}

#[derive(Deserialize)]
struct CfDnsRecord {
    id: String,
    #[serde(rename = "type")]
    record_type: String,
    name: String,
    content: String,
    ttl: i32,
    #[serde(default)]
    proxied: bool,
}

#[derive(Deserialize)]
struct CfCertPack {
    certificates: Option<Vec<CfCert>>,
}

#[derive(Deserialize)]
struct CfCert {
    expires_on: Option<String>,
}

#[derive(Deserialize)]
struct CfRegistrarDomain {
    expires_at: Option<String>,
}

fn mask_token(token: &str) -> String {
    if token.len() <= 8 {
        "****".into()
    } else {
        format!("{}...{}", &token[..4], &token[token.len() - 4..])
    }
}

// ─── Account CRUD ─────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/cloudflare-accounts",
    tag = "Cloudflare",
    params(CloudflareAccountListQuery),
    responses((status = 200, body = PaginatedCloudflareAccountResponse))
)]
pub async fn list_accounts(
    State(state): State<AppState>,
    Query(q): Query<CloudflareAccountListQuery>,
) -> Result<Json<PaginatedResponse<CloudflareAccountResponse>>, AppError> {
    let per_page = q.per_page.clamp(1, 100);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cloudflare_accounts")
        .fetch_one(&state.db)
        .await?;

    let rows = sqlx::query_as::<_, CloudflareAccount>(
        "SELECT * FROM cloudflare_accounts ORDER BY name LIMIT $1 OFFSET $2",
    )
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    let mut data = Vec::with_capacity(rows.len());
    for acc in &rows {
        let (zones_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM cloudflare_zones WHERE account_id = $1")
                .bind(acc.id)
                .fetch_one(&state.db)
                .await?;
        let (records_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM cloudflare_dns_records r JOIN cloudflare_zones z ON z.id = r.zone_uuid WHERE z.account_id = $1",
        )
        .bind(acc.id)
        .fetch_one(&state.db)
        .await?;
        let last_synced: (Option<DateTime<Utc>>,) = sqlx::query_as(
            "SELECT MAX(synced_at) FROM cloudflare_zones WHERE account_id = $1",
        )
        .bind(acc.id)
        .fetch_one(&state.db)
        .await?;

        data.push(CloudflareAccountResponse {
            id: acc.id,
            name: acc.name.clone(),
            masked_token: mask_token(&acc.api_token),
            zones_count,
            records_count,
            last_synced: last_synced.0,
            created_at: acc.created_at,
            updated_at: acc.updated_at,
        });
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
    path = "/api/cloudflare-accounts/{id}",
    tag = "Cloudflare",
    params(("id" = Uuid, Path, description = "Account ID")),
    responses(
        (status = 200, body = CloudflareAccountResponse),
        (status = 404),
    )
)]
pub async fn get_account(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CloudflareAccountResponse>, AppError> {
    let acc = sqlx::query_as::<_, CloudflareAccount>(
        "SELECT * FROM cloudflare_accounts WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let (zones_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM cloudflare_zones WHERE account_id = $1")
            .bind(acc.id)
            .fetch_one(&state.db)
            .await?;
    let (records_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM cloudflare_dns_records r JOIN cloudflare_zones z ON z.id = r.zone_uuid WHERE z.account_id = $1",
    )
    .bind(acc.id)
    .fetch_one(&state.db)
    .await?;
    let last_synced: (Option<DateTime<Utc>>,) = sqlx::query_as(
        "SELECT MAX(synced_at) FROM cloudflare_zones WHERE account_id = $1",
    )
    .bind(acc.id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(CloudflareAccountResponse {
        id: acc.id,
        name: acc.name,
        masked_token: mask_token(&acc.api_token),
        zones_count,
        records_count,
        last_synced: last_synced.0,
        created_at: acc.created_at,
        updated_at: acc.updated_at,
    }))
}

#[utoipa::path(
    post,
    path = "/api/cloudflare-accounts",
    tag = "Cloudflare",
    request_body = CreateCloudflareAccount,
    responses(
        (status = 200, body = CloudflareAccountResponse),
        (status = 400),
    )
)]
pub async fn create_account(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Json(input): Json<CreateCloudflareAccount>,
) -> Result<Json<CloudflareAccountResponse>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    // Validate token by calling CF API
    let resp = state
        .http_client
        .get("https://api.cloudflare.com/client/v4/user/tokens/verify")
        .header("Authorization", format!("Bearer {}", &input.api_token))
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Failed to verify token: {e}")))?;

    if !resp.status().is_success() {
        return Err(AppError::BadRequest(
            "Invalid Cloudflare API token".into(),
        ));
    }

    let acc = sqlx::query_as::<_, CloudflareAccount>(
        "INSERT INTO cloudflare_accounts (name, api_token) VALUES ($1, $2) RETURNING *",
    )
    .bind(&input.name)
    .bind(&input.api_token)
    .fetch_one(&state.db)
    .await?;

    log_audit(
        &state.db,
        &user,
        "create",
        "cloudflare_account",
        Some(&acc.id.to_string()),
        serde_json::json!({"name": acc.name}),
    )
    .await;

    Ok(Json(CloudflareAccountResponse {
        id: acc.id,
        name: acc.name,
        masked_token: mask_token(&acc.api_token),
        zones_count: 0,
        records_count: 0,
        last_synced: None,
        created_at: acc.created_at,
        updated_at: acc.updated_at,
    }))
}

#[utoipa::path(
    put,
    path = "/api/cloudflare-accounts/{id}",
    tag = "Cloudflare",
    params(("id" = Uuid, Path, description = "Account ID")),
    request_body = UpdateCloudflareAccount,
    responses(
        (status = 200, body = CloudflareAccountResponse),
        (status = 404),
    )
)]
pub async fn update_account(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateCloudflareAccount>,
) -> Result<Json<CloudflareAccountResponse>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    let existing = sqlx::query_as::<_, CloudflareAccount>(
        "SELECT * FROM cloudflare_accounts WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let new_name = input.name.unwrap_or(existing.name);
    let new_token = input.api_token.unwrap_or(existing.api_token);

    let acc = sqlx::query_as::<_, CloudflareAccount>(
        "UPDATE cloudflare_accounts SET name = $2, api_token = $3, updated_at = now() WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(&new_name)
    .bind(&new_token)
    .fetch_one(&state.db)
    .await?;

    log_audit(
        &state.db,
        &user,
        "update",
        "cloudflare_account",
        Some(&id.to_string()),
        serde_json::json!({"name": acc.name}),
    )
    .await;

    // Return with stats
    let (zones_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM cloudflare_zones WHERE account_id = $1")
            .bind(acc.id)
            .fetch_one(&state.db)
            .await?;
    let (records_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM cloudflare_dns_records r JOIN cloudflare_zones z ON z.id = r.zone_uuid WHERE z.account_id = $1",
    )
    .bind(acc.id)
    .fetch_one(&state.db)
    .await?;
    let last_synced: (Option<DateTime<Utc>>,) = sqlx::query_as(
        "SELECT MAX(synced_at) FROM cloudflare_zones WHERE account_id = $1",
    )
    .bind(acc.id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(CloudflareAccountResponse {
        id: acc.id,
        name: acc.name,
        masked_token: mask_token(&acc.api_token),
        zones_count,
        records_count,
        last_synced: last_synced.0,
        created_at: acc.created_at,
        updated_at: acc.updated_at,
    }))
}

#[utoipa::path(
    delete,
    path = "/api/cloudflare-accounts/{id}",
    tag = "Cloudflare",
    params(("id" = Uuid, Path, description = "Account ID")),
    responses((status = 200), (status = 404))
)]
pub async fn delete_account(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    let result = sqlx::query("DELETE FROM cloudflare_accounts WHERE id = $1")
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
        "cloudflare_account",
        Some(&id.to_string()),
        serde_json::json!({}),
    )
    .await;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

// ─── Sync ─────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/api/cloudflare-accounts/{id}/sync",
    tag = "Cloudflare",
    params(("id" = Uuid, Path, description = "Account ID")),
    responses(
        (status = 200, body = CloudflareSyncResult),
        (status = 404),
    )
)]
pub async fn sync_account(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<CloudflareSyncResult>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    let acc = sqlx::query_as::<_, CloudflareAccount>(
        "SELECT * FROM cloudflare_accounts WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let client = &state.http_client;
    let token = &acc.api_token;

    // 1. Fetch all zones (paginated)
    let mut all_zones: Vec<CfZone> = Vec::new();
    let mut page = 1u32;
    loop {
        let resp = client
            .get(format!(
                "https://api.cloudflare.com/client/v4/zones?per_page=50&page={page}"
            ))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("CF API error: {e}")))?;

        let body = resp
            .json::<CfResponse<Vec<CfZone>>>()
            .await
            .map_err(|e| AppError::Internal(format!("CF parse error: {e}")))?;

        if !body.success {
            return Err(AppError::Internal("CF API returned success=false".into()));
        }

        all_zones.extend(body.result);
        match body.result_info {
            Some(info) if page < info.total_pages => page += 1,
            _ => break,
        }
    }

    // Build a map of all VPS IPs for matching
    let vps_rows: Vec<(Uuid, serde_json::Value)> =
        sqlx::query_as("SELECT id, ip_addresses FROM vps WHERE status != 'retired'")
            .fetch_all(&state.db)
            .await?;

    let mut ip_to_vps: std::collections::HashMap<String, Uuid> =
        std::collections::HashMap::new();
    for (vps_id, ips_json) in &vps_rows {
        if let Ok(ips) = serde_json::from_value::<Vec<crate::models::IpEntry>>(ips_json.clone()) {
            for entry in ips {
                ip_to_vps.insert(entry.ip.clone(), *vps_id);
            }
        }
    }

    let now = Utc::now();
    let mut seen_zone_ids: Vec<String> = Vec::new();
    let mut total_records: i64 = 0;
    let mut total_linked: i64 = 0;
    let mut total_deleted: i64 = 0;

    for zone in &all_zones {
        seen_zone_ids.push(zone.id.clone());

        // Upsert zone
        let zone_uuid: (Uuid,) = sqlx::query_as(
            r#"INSERT INTO cloudflare_zones (account_id, zone_id, zone_name, status, synced_at)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (account_id, zone_id)
               DO UPDATE SET zone_name = $3, status = $4, synced_at = $5, updated_at = now()
               RETURNING id"#,
        )
        .bind(acc.id)
        .bind(&zone.id)
        .bind(&zone.name)
        .bind(&zone.status)
        .bind(now)
        .fetch_one(&state.db)
        .await?;

        // Fetch DNS records for this zone
        let dns_resp = client
            .get(format!(
                "https://api.cloudflare.com/client/v4/zones/{}/dns_records?per_page=5000",
                zone.id
            ))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await;

        let mut seen_record_ids: Vec<String> = Vec::new();

        if let Ok(resp) = dns_resp {
            if let Ok(body) = resp.json::<CfResponse<Vec<CfDnsRecord>>>().await {
                for rec in body.result {
                    seen_record_ids.push(rec.id.clone());

                    let vps_id = if rec.record_type == "A" || rec.record_type == "AAAA" {
                        ip_to_vps.get(&rec.content).copied()
                    } else {
                        None
                    };

                    if vps_id.is_some() {
                        total_linked += 1;
                    }

                    sqlx::query(
                        r#"INSERT INTO cloudflare_dns_records (zone_uuid, record_id, record_type, name, content, ttl, proxied, vps_id, synced_at)
                           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                           ON CONFLICT (zone_uuid, record_id)
                           DO UPDATE SET record_type = $3, name = $4, content = $5, ttl = $6, proxied = $7, vps_id = $8, synced_at = $9, updated_at = now()"#,
                    )
                    .bind(zone_uuid.0)
                    .bind(&rec.id)
                    .bind(&rec.record_type)
                    .bind(&rec.name)
                    .bind(&rec.content)
                    .bind(rec.ttl)
                    .bind(rec.proxied)
                    .bind(vps_id)
                    .bind(now)
                    .execute(&state.db)
                    .await?;

                    total_records += 1;
                }
            }
        }

        // Delete stale records for this zone
        if !seen_record_ids.is_empty() {
            let deleted = sqlx::query(
                "DELETE FROM cloudflare_dns_records WHERE zone_uuid = $1 AND record_id != ALL($2)",
            )
            .bind(zone_uuid.0)
            .bind(&seen_record_ids)
            .execute(&state.db)
            .await?;
            total_deleted += deleted.rows_affected() as i64;
        } else {
            // If no records fetched, delete all records for this zone
            let deleted =
                sqlx::query("DELETE FROM cloudflare_dns_records WHERE zone_uuid = $1")
                    .bind(zone_uuid.0)
                    .execute(&state.db)
                    .await?;
            total_deleted += deleted.rows_affected() as i64;
        }

        // Best-effort: fetch cert expiry
        let cert_expires = fetch_cert_expiry(client, token, &zone.id).await;
        if let Some(expires) = cert_expires {
            let _ = sqlx::query(
                "UPDATE cloudflare_zones SET cert_expires_at = $2 WHERE id = $1",
            )
            .bind(zone_uuid.0)
            .bind(expires)
            .execute(&state.db)
            .await;
        }

        // Best-effort: fetch domain expiry (CF Registrar)
        let domain_expires =
            fetch_domain_expiry(client, token, &zone.account.id, &zone.name).await;
        if let Some(expires) = domain_expires {
            let _ = sqlx::query(
                "UPDATE cloudflare_zones SET domain_expires_at = $2 WHERE id = $1",
            )
            .bind(zone_uuid.0)
            .bind(expires)
            .execute(&state.db)
            .await;
        }
    }

    // Delete stale zones not seen in this sync
    if !seen_zone_ids.is_empty() {
        let deleted = sqlx::query(
            "DELETE FROM cloudflare_zones WHERE account_id = $1 AND zone_id != ALL($2)",
        )
        .bind(acc.id)
        .bind(&seen_zone_ids)
        .execute(&state.db)
        .await?;
        total_deleted += deleted.rows_affected() as i64;
    }

    log_audit(
        &state.db,
        &user,
        "sync",
        "cloudflare_account",
        Some(&id.to_string()),
        serde_json::json!({
            "zones": all_zones.len(),
            "records": total_records,
            "linked": total_linked,
        }),
    )
    .await;

    Ok(Json(CloudflareSyncResult {
        zones_count: all_zones.len() as i64,
        records_count: total_records,
        records_linked: total_linked,
        records_deleted: total_deleted,
    }))
}

async fn fetch_cert_expiry(
    client: &reqwest::Client,
    token: &str,
    zone_id: &str,
) -> Option<DateTime<Utc>> {
    let resp = client
        .get(format!(
            "https://api.cloudflare.com/client/v4/zones/{zone_id}/ssl/certificate_packs"
        ))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .ok()?;

    let body = resp.json::<CfResponse<Vec<CfCertPack>>>().await.ok()?;
    let mut earliest: Option<DateTime<Utc>> = None;

    for pack in body.result {
        if let Some(certs) = pack.certificates {
            for cert in certs {
                if let Some(ref exp) = cert.expires_on {
                    if let Ok(dt) = exp.parse::<DateTime<Utc>>() {
                        earliest = Some(match earliest {
                            Some(e) if dt < e => dt,
                            None => dt,
                            Some(e) => e,
                        });
                    }
                }
            }
        }
    }

    earliest
}

async fn fetch_domain_expiry(
    client: &reqwest::Client,
    token: &str,
    cf_account_id: &str,
    domain: &str,
) -> Option<DateTime<Utc>> {
    let resp = client
        .get(format!(
            "https://api.cloudflare.com/client/v4/accounts/{cf_account_id}/registrar/domains/{domain}"
        ))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let body = resp
        .json::<CfResponse<CfRegistrarDomain>>()
        .await
        .ok()?;
    let exp_str = body.result.expires_at?;
    exp_str.parse::<DateTime<Utc>>().ok()
}

// ─── Zones ────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/cloudflare-zones",
    tag = "Cloudflare",
    params(CloudflareZoneListQuery),
    responses((status = 200, body = PaginatedCloudflareZoneResponse))
)]
pub async fn list_zones(
    State(state): State<AppState>,
    Query(q): Query<CloudflareZoneListQuery>,
) -> Result<Json<PaginatedResponse<CloudflareZone>>, AppError> {
    let per_page = q.per_page.clamp(1, 100);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let mut where_clause = String::from(" WHERE 1=1");
    let mut param_idx = 0u32;

    if q.account_id.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND account_id = ${param_idx}"));
    }

    let count_sql = format!("SELECT COUNT(*) FROM cloudflare_zones{where_clause}");
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
        "SELECT * FROM cloudflare_zones{where_clause} ORDER BY zone_name LIMIT ${limit_param} OFFSET ${offset_param}"
    );
    let mut query = sqlx::query_as::<_, CloudflareZone>(&data_sql);
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
    path = "/api/cloudflare-dns-records",
    tag = "Cloudflare",
    params(CloudflareDnsListQuery),
    responses((status = 200, body = PaginatedCloudflareDnsResponse))
)]
pub async fn list_dns_records(
    State(state): State<AppState>,
    Query(q): Query<CloudflareDnsListQuery>,
) -> Result<Json<PaginatedResponse<CloudflareDnsRecord>>, AppError> {
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
        "SELECT COUNT(*) FROM cloudflare_dns_records d JOIN cloudflare_zones z ON z.id = d.zone_uuid{where_clause}"
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
                  d.ttl, d.proxied, d.vps_id, d.synced_at, d.created_at, d.updated_at,
                  z.zone_name, z.zone_id AS zone_id_cf,
                  v.hostname AS vps_hostname, v.country AS vps_country
           FROM cloudflare_dns_records d
           JOIN cloudflare_zones z ON z.id = d.zone_uuid
           LEFT JOIN vps v ON v.id = d.vps_id
           {where_clause}
           ORDER BY z.zone_name, d.name
           LIMIT ${limit_param} OFFSET ${offset_param}"#
    );

    let mut query = sqlx::query_as::<_, CloudflareDnsRecord>(&data_sql);
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
