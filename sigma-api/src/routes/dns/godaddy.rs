use serde::Deserialize;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{DnsAccount, DnsSyncResult, IpEntry};
use crate::routes::AppState;

const GODADDY_API_BASE: &str = "https://api.godaddy.com/v1";

// ─── GoDaddy API response structs ────────────────────────

#[derive(Deserialize)]
struct GdDomain {
    domain: String,
    status: String,
    expires: Option<String>,
}

#[derive(Deserialize)]
struct GdDnsRecord {
    #[serde(rename = "type")]
    record_type: String,
    name: String,
    data: String,
    ttl: i32,
}

fn auth_header(config: &serde_json::Value) -> Result<String, AppError> {
    let api_key = config
        .get("api_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing api_key in config".into()))?;
    let api_secret = config
        .get("api_secret")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing api_secret in config".into()))?;
    Ok(format!("sso-key {api_key}:{api_secret}"))
}

/// Validate GoDaddy credentials by fetching domain list.
pub async fn validate(
    http_client: &reqwest::Client,
    config: &serde_json::Value,
) -> Result<(), AppError> {
    let auth = auth_header(config)?;

    let resp = http_client
        .get(format!("{GODADDY_API_BASE}/domains?limit=1"))
        .header("Authorization", &auth)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("GoDaddy API error: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "GoDaddy auth failed (HTTP {status}): {body}"
        )));
    }

    Ok(())
}

/// Mask sensitive fields in GoDaddy config.
pub fn mask_config(config: &serde_json::Value) -> serde_json::Value {
    let api_key = config
        .get("api_key")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    serde_json::json!({
        "api_key": mask_key(api_key),
        "api_secret": "****",
    })
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "****".into()
    } else {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    }
}

/// Full sync: fetch all domains and DNS records from GoDaddy.
pub async fn sync(state: &AppState, account: &DnsAccount) -> Result<DnsSyncResult, AppError> {
    let client = &state.http_client;
    let auth = auth_header(&account.config)?;

    // Fetch all domains
    let resp = client
        .get(format!("{GODADDY_API_BASE}/domains?limit=999"))
        .header("Authorization", &auth)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("GoDaddy API error: {e}")))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Internal(format!("GoDaddy list domains failed: {body}")));
    }

    let domains: Vec<GdDomain> = resp
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("GoDaddy parse error: {e}")))?;

    // Build IP→VPS map
    let vps_rows: Vec<(Uuid, serde_json::Value)> =
        sqlx::query_as("SELECT id, ip_addresses FROM vps WHERE status != 'retired'")
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

    let now = chrono::Utc::now();
    let mut seen_zone_ids: Vec<String> = Vec::new();
    let mut total_records: i64 = 0;
    let mut total_linked: i64 = 0;
    let mut total_deleted: i64 = 0;

    for domain in &domains {
        seen_zone_ids.push(domain.domain.clone());

        // Parse domain expiry
        let domain_expires = domain
            .expires
            .as_deref()
            .and_then(|s| s.parse::<chrono::DateTime<chrono::Utc>>().ok());

        // Upsert zone
        let zone_uuid: (Uuid,) = sqlx::query_as(
            r#"INSERT INTO dns_zones (account_id, zone_id, zone_name, status, domain_expires_at, synced_at)
               VALUES ($1, $2, $3, $4, $5, $6)
               ON CONFLICT (account_id, zone_id)
               DO UPDATE SET zone_name = $3, status = $4, domain_expires_at = COALESCE($5, dns_zones.domain_expires_at), synced_at = $6, updated_at = now()
               RETURNING id"#,
        )
        .bind(account.id)
        .bind(&domain.domain)
        .bind(&domain.domain)
        .bind(&domain.status)
        .bind(domain_expires)
        .bind(now)
        .fetch_one(&state.db)
        .await?;

        // Fetch DNS records for this domain
        let records_resp = client
            .get(format!(
                "{GODADDY_API_BASE}/domains/{}/records",
                domain.domain
            ))
            .header("Authorization", &auth)
            .send()
            .await;

        let mut seen_record_ids: Vec<String> = Vec::new();

        if let Ok(resp) = records_resp {
            if let Ok(records) = resp.json::<Vec<GdDnsRecord>>().await {
                for (i, rec) in records.iter().enumerate() {
                    // GoDaddy doesn't have individual record IDs, so build a composite key
                    let record_id = format!("{}:{}:{}", rec.name, rec.record_type, i);
                    seen_record_ids.push(record_id.clone());

                    // Build full record name
                    let full_name = if rec.name == "@" {
                        domain.domain.clone()
                    } else {
                        format!("{}.{}", rec.name, domain.domain)
                    };

                    let vps_id = if rec.record_type == "A" || rec.record_type == "AAAA" {
                        ip_to_vps.get(&rec.data).copied()
                    } else {
                        None
                    };

                    if vps_id.is_some() {
                        total_linked += 1;
                    }

                    sqlx::query(
                        r#"INSERT INTO dns_records (zone_uuid, record_id, record_type, name, content, ttl, extra, vps_id, synced_at)
                           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                           ON CONFLICT (zone_uuid, record_id)
                           DO UPDATE SET record_type = $3, name = $4, content = $5, ttl = $6, extra = $7, vps_id = $8, synced_at = $9, updated_at = now()"#,
                    )
                    .bind(zone_uuid.0)
                    .bind(&record_id)
                    .bind(&rec.record_type)
                    .bind(&full_name)
                    .bind(&rec.data)
                    .bind(rec.ttl)
                    .bind(serde_json::json!({}))
                    .bind(vps_id)
                    .bind(now)
                    .execute(&state.db)
                    .await?;

                    total_records += 1;
                }
            }
        }

        // Delete stale records
        if !seen_record_ids.is_empty() {
            let deleted = sqlx::query(
                "DELETE FROM dns_records WHERE zone_uuid = $1 AND record_id != ALL($2)",
            )
            .bind(zone_uuid.0)
            .bind(&seen_record_ids)
            .execute(&state.db)
            .await?;
            total_deleted += deleted.rows_affected() as i64;
        } else {
            let deleted = sqlx::query("DELETE FROM dns_records WHERE zone_uuid = $1")
                .bind(zone_uuid.0)
                .execute(&state.db)
                .await?;
            total_deleted += deleted.rows_affected() as i64;
        }
    }

    // Delete stale zones
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

    Ok(DnsSyncResult {
        zones_count: domains.len() as i64,
        records_count: total_records,
        records_linked: total_linked,
        records_deleted: total_deleted,
    })
}
