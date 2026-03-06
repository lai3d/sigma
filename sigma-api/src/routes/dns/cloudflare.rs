use chrono::{DateTime, Utc};
use futures::stream::{self, StreamExt};
use serde::Deserialize;
use tracing::warn;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{DnsAccount, DnsSyncResult, IpEntry};
use crate::routes::AppState;

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

// ─── Cloudflare Audit Log structs ───────────────────────

#[derive(Deserialize)]
struct CfAuditLogResponse {
    result: Vec<CfAuditLogEntry>,
    result_info: Option<CfAuditLogResultInfo>,
    success: bool,
}

#[derive(Deserialize)]
struct CfAuditLogResultInfo {
    #[allow(dead_code)]
    page: u32,
    total_pages: u32,
}

#[derive(Deserialize)]
struct CfAuditLogEntry {
    action: CfAuditAction,
    actor: CfAuditActor,
    resource: CfAuditResource,
}

#[derive(Deserialize)]
struct CfAuditAction {
    #[serde(rename = "type")]
    action_type: String,
}

#[derive(Deserialize)]
struct CfAuditActor {
    email: Option<String>,
    ip: Option<String>,
}

#[derive(Deserialize)]
struct CfAuditResource {
    id: Option<String>,
}

struct AuditInfo {
    email: Option<String>,
    ip: Option<String>,
}

/// Fetch Cloudflare audit logs for DNS record actions since the given timestamp.
/// Returns empty Vec on permission errors (403) or any failure.
async fn fetch_audit_logs(
    client: &reqwest::Client,
    token: &str,
    cf_account_id: &str,
    since: &DateTime<Utc>,
) -> Vec<CfAuditLogEntry> {
    let since_str = since.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let mut all_entries = Vec::new();
    let mut page = 1u32;

    loop {
        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{cf_account_id}/audit_logs?since={since_str}&per_page=100&page={page}"
        );

        let resp = match client
            .get(&url)
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("audit log fetch failed: {e}");
                return all_entries;
            }
        };

        if resp.status() == reqwest::StatusCode::FORBIDDEN {
            warn!("audit log fetch: 403 forbidden (token lacks Audit Logs Read permission)");
            return Vec::new();
        }

        if !resp.status().is_success() {
            warn!("audit log fetch: HTTP {}", resp.status());
            return all_entries;
        }

        let body: CfAuditLogResponse = match resp.json().await {
            Ok(b) => b,
            Err(e) => {
                warn!("audit log parse error: {e}");
                return all_entries;
            }
        };

        if !body.success {
            warn!("audit log API returned success=false");
            return all_entries;
        }

        // Filter to DNS record actions only
        let dns_entries: Vec<CfAuditLogEntry> = body
            .result
            .into_iter()
            .filter(|e| matches!(e.action.action_type.as_str(), "rec_add" | "rec_set" | "rec_del"))
            .collect();

        all_entries.extend(dns_entries);

        match body.result_info {
            Some(info) if page < info.total_pages => page += 1,
            _ => break,
        }
    }

    all_entries
}

/// Map Cloudflare audit action types to our history action names.
fn cf_action_to_history_action(cf_action: &str) -> Option<&'static str> {
    match cf_action {
        "rec_add" => Some("created"),
        "rec_set" => Some("updated"),
        "rec_del" => Some("deleted"),
        _ => None,
    }
}

/// After sync, enrich dns_record_history rows with actor info from Cloudflare audit logs.
async fn enrich_history_with_audit_logs(
    db: &sqlx::PgPool,
    client: &reqwest::Client,
    token: &str,
    cf_account_id: &str,
    since: &DateTime<Utc>,
) {
    let entries = fetch_audit_logs(client, token, cf_account_id, since).await;
    if entries.is_empty() {
        return;
    }

    // Build lookup: (cf_record_id, history_action) → AuditInfo
    let mut lookup: std::collections::HashMap<(String, String), AuditInfo> =
        std::collections::HashMap::new();

    for entry in entries {
        if let (Some(record_id), Some(action)) = (
            entry.resource.id,
            cf_action_to_history_action(&entry.action.action_type),
        ) {
            lookup.insert(
                (record_id, action.to_string()),
                AuditInfo {
                    email: entry.actor.email,
                    ip: entry.actor.ip,
                },
            );
        }
    }

    let mut enriched: i64 = 0;
    for ((record_id, action), info) in &lookup {
        let result = sqlx::query(
            r#"UPDATE dns_record_history
               SET actor_email = $1, actor_ip = $2
               WHERE record_id = $3 AND action = $4
                 AND actor_email IS NULL
                 AND created_at >= $5"#,
        )
        .bind(&info.email)
        .bind(&info.ip)
        .bind(record_id)
        .bind(action)
        .bind(since)
        .execute(db)
        .await;

        if let Ok(r) = result {
            enriched += r.rows_affected() as i64;
        }
    }

    tracing::info!("audit log enrichment: processed {enriched} entries from {} audit records", lookup.len());
}

/// Validate Cloudflare credentials by calling the token verify endpoint.
pub async fn validate(
    http_client: &reqwest::Client,
    config: &serde_json::Value,
) -> Result<(), AppError> {
    let api_token = config
        .get("api_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing api_token in config".into()))?;

    let resp = http_client
        .get("https://api.cloudflare.com/client/v4/user/tokens/verify")
        .header("Authorization", format!("Bearer {api_token}"))
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Failed to verify token: {e}")))?;

    if !resp.status().is_success() {
        return Err(AppError::BadRequest(
            "Invalid Cloudflare API token".into(),
        ));
    }

    Ok(())
}

/// Mask sensitive fields in Cloudflare config.
pub fn mask_config(config: &serde_json::Value) -> serde_json::Value {
    let token = config
        .get("api_token")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    serde_json::json!({
        "api_token": mask_token(token),
    })
}

fn mask_token(token: &str) -> String {
    if token.len() <= 8 {
        "****".into()
    } else {
        format!("{}...{}", &token[..4], &token[token.len() - 4..])
    }
}

/// Full sync: fetch all zones and DNS records from Cloudflare, upsert into DB.
pub async fn sync(state: &AppState, account: &DnsAccount) -> Result<DnsSyncResult, AppError> {
    let client = &state.http_client;
    let token = account
        .config
        .get("api_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Internal("Missing api_token in account config".into()))?;

    let sync_start = Utc::now();

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
        sqlx::query_as("SELECT id, ip_addresses FROM vps WHERE status NOT IN ('retired', 'deleted')")
            .fetch_all(&state.db)
            .await?;

    let mut ip_to_vps: std::collections::HashMap<String, Uuid> =
        std::collections::HashMap::new();
    for (vps_id, ips_json) in &vps_rows {
        if let Ok(ips) = serde_json::from_value::<Vec<IpEntry>>(ips_json.clone()) {
            for entry in ips {
                ip_to_vps.insert(entry.ip.clone(), *vps_id);
            }
        }
    }

    let now = Utc::now();
    let zones_count = all_zones.len() as i64;
    let seen_zone_ids: Vec<String> = all_zones.iter().map(|z| z.id.clone()).collect();
    let cf_account_id_from_zones: Option<String> =
        all_zones.first().map(|z| z.account.id.clone());

    // Process zones concurrently (up to 10 at a time)
    let zone_results: Vec<Result<ZoneSyncResult, AppError>> = stream::iter(all_zones)
        .map(|zone| {
            let db = state.db.clone();
            let client = client.clone();
            let token = token.to_string();
            let ip_to_vps = &ip_to_vps;
            let account_id = account.id;
            async move {
                sync_zone(&db, &client, &token, ip_to_vps, account_id, &zone, now).await
            }
        })
        .buffer_unordered(10)
        .collect()
        .await;

    let mut total_records: i64 = 0;
    let mut total_linked: i64 = 0;
    let mut total_deleted: i64 = 0;

    for result in zone_results {
        let zr = result?;
        total_records += zr.records;
        total_linked += zr.linked;
        total_deleted += zr.deleted;
    }

    // Delete stale zones not seen in this sync
    if !seen_zone_ids.is_empty() {
        let deleted = sqlx::query(
            "DELETE FROM dns_zones WHERE account_id = $1 AND zone_id != ALL($2)",
        )
        .bind(account.id)
        .bind(&seen_zone_ids)
        .execute(&state.db)
        .await?;
        total_deleted += deleted.rows_affected() as i64;
    }

    // Enrich history with audit logs (best-effort, Cloudflare only)
    if let Some(cf_account_id) = cf_account_id_from_zones.as_deref() {
        enrich_history_with_audit_logs(&state.db, client, token, cf_account_id, &sync_start)
            .await;
    }

    Ok(DnsSyncResult {
        zones_count,
        records_count: total_records,
        records_linked: total_linked,
        records_deleted: total_deleted,
    })
}

/// Sync a single zone by its Cloudflare zone ID.
pub async fn sync_single_zone(
    state: &AppState,
    account: &DnsAccount,
    zone_cf_id: &str,
    _zone_name: &str,
) -> Result<DnsSyncResult, AppError> {
    let client = &state.http_client;
    let token = account
        .config
        .get("api_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Internal("Missing api_token in account config".into()))?;

    let sync_start = Utc::now();

    // Fetch zone info from CF to get the account ref (needed for registrar API)
    let resp = client
        .get(format!(
            "https://api.cloudflare.com/client/v4/zones/{zone_cf_id}"
        ))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("CF API error: {e}")))?;

    let body = resp
        .json::<CfResponse<CfZone>>()
        .await
        .map_err(|e| AppError::Internal(format!("CF parse error: {e}")))?;

    if !body.success {
        return Err(AppError::Internal("CF API returned success=false".into()));
    }

    let zone = body.result;

    // Build IP→VPS map
    let vps_rows: Vec<(Uuid, serde_json::Value)> =
        sqlx::query_as("SELECT id, ip_addresses FROM vps WHERE status NOT IN ('retired', 'deleted')")
            .fetch_all(&state.db)
            .await?;

    let mut ip_to_vps: std::collections::HashMap<String, Uuid> =
        std::collections::HashMap::new();
    for (vps_id, ips_json) in &vps_rows {
        if let Ok(ips) = serde_json::from_value::<Vec<IpEntry>>(ips_json.clone()) {
            for entry in ips {
                ip_to_vps.insert(entry.ip.clone(), *vps_id);
            }
        }
    }

    let now = Utc::now();
    let zr = sync_zone(
        &state.db, client, token, &ip_to_vps, account.id, &zone, now,
    )
    .await?;

    // Enrich history with audit logs (best-effort)
    enrich_history_with_audit_logs(&state.db, client, token, &zone.account.id, &sync_start)
        .await;

    Ok(DnsSyncResult {
        zones_count: 1,
        records_count: zr.records,
        records_linked: zr.linked,
        records_deleted: zr.deleted,
    })
}

struct ZoneSyncResult {
    records: i64,
    linked: i64,
    deleted: i64,
}

/// Process a single zone: upsert zone, fetch records + cert + domain concurrently, upsert records.
async fn sync_zone(
    db: &sqlx::PgPool,
    client: &reqwest::Client,
    token: &str,
    ip_to_vps: &std::collections::HashMap<String, Uuid>,
    account_id: Uuid,
    zone: &CfZone,
    now: DateTime<Utc>,
) -> Result<ZoneSyncResult, AppError> {
    // Upsert zone
    let zone_uuid: (Uuid,) = sqlx::query_as(
        r#"INSERT INTO dns_zones (account_id, zone_id, zone_name, status, synced_at)
           VALUES ($1, $2, $3, $4, $5)
           ON CONFLICT (account_id, zone_id)
           DO UPDATE SET zone_name = $3, status = $4, synced_at = $5, updated_at = now()
           RETURNING id"#,
    )
    .bind(account_id)
    .bind(&zone.id)
    .bind(&zone.name)
    .bind(&zone.status)
    .bind(now)
    .fetch_one(db)
    .await?;

    // Fetch DNS records, cert expiry, and domain expiry concurrently
    let records_fut = client
        .get(format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records?per_page=5000",
            zone.id
        ))
        .header("Authorization", format!("Bearer {token}"))
        .send();
    let cert_fut = fetch_cert_expiry(client, token, &zone.id);
    let domain_fut = fetch_domain_expiry(client, token, &zone.account.id, &zone.name);

    let (dns_resp, cert_expires, domain_expires) =
        tokio::join!(records_fut, cert_fut, domain_fut);

    // Process DNS records
    let mut seen_record_ids: Vec<String> = Vec::new();
    let mut records: i64 = 0;
    let mut linked: i64 = 0;

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
                    linked += 1;
                }

                let extra = serde_json::json!({"proxied": rec.proxied});

                sqlx::query(
                    r#"INSERT INTO dns_records (zone_uuid, record_id, record_type, name, content, ttl, extra, vps_id, synced_at)
                       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                       ON CONFLICT (zone_uuid, record_id)
                       DO UPDATE SET record_type = $3, name = $4, content = $5, ttl = $6, extra = $7, vps_id = $8, synced_at = $9, updated_at = now()"#,
                )
                .bind(zone_uuid.0)
                .bind(&rec.id)
                .bind(&rec.record_type)
                .bind(&rec.name)
                .bind(&rec.content)
                .bind(rec.ttl)
                .bind(&extra)
                .bind(vps_id)
                .bind(now)
                .execute(db)
                .await?;

                records += 1;
            }
        }
    }

    // Delete stale records for this zone
    let deleted = if !seen_record_ids.is_empty() {
        sqlx::query(
            "DELETE FROM dns_records WHERE zone_uuid = $1 AND record_id != ALL($2)",
        )
        .bind(zone_uuid.0)
        .bind(&seen_record_ids)
        .execute(db)
        .await?
        .rows_affected() as i64
    } else {
        sqlx::query("DELETE FROM dns_records WHERE zone_uuid = $1")
            .bind(zone_uuid.0)
            .execute(db)
            .await?
            .rows_affected() as i64
    };

    // Update cert/domain expiry (best-effort)
    if let Some(expires) = cert_expires {
        let _ = sqlx::query("UPDATE dns_zones SET cert_expires_at = $2 WHERE id = $1")
            .bind(zone_uuid.0)
            .bind(expires)
            .execute(db)
            .await;
    }
    if let Some(expires) = domain_expires {
        let _ = sqlx::query("UPDATE dns_zones SET domain_expires_at = $2 WHERE id = $1")
            .bind(zone_uuid.0)
            .bind(expires)
            .execute(db)
            .await;
    }

    Ok(ZoneSyncResult {
        records,
        linked,
        deleted,
    })
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
