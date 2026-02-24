use serde::Deserialize;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{DnsAccount, DnsSyncResult, IpEntry};
use crate::routes::AppState;

const NAMECOM_API_BASE: &str = "https://api.name.com/v4";

// ─── Name.com API response structs ───────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NcListDomainsResponse {
    domains: Option<Vec<NcDomain>>,
    next_page: Option<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NcDomain {
    domain_name: String,
    expire_date: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NcListRecordsResponse {
    records: Option<Vec<NcRecord>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NcRecord {
    id: i64,
    #[serde(rename = "type")]
    record_type: String,
    #[allow(dead_code)]
    host: String,
    answer: String,
    ttl: i32,
    fqdn: String,
}

fn basic_auth(config: &serde_json::Value) -> Result<(String, String), AppError> {
    let username = config
        .get("username")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing username in config".into()))?;
    let api_token = config
        .get("api_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing api_token in config".into()))?;
    Ok((username.to_string(), api_token.to_string()))
}

/// Validate Name.com credentials by fetching domain list.
pub async fn validate(
    http_client: &reqwest::Client,
    config: &serde_json::Value,
) -> Result<(), AppError> {
    let (username, api_token) = basic_auth(config)?;

    let resp = http_client
        .get(format!("{NAMECOM_API_BASE}/domains?perPage=1"))
        .basic_auth(&username, Some(&api_token))
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Name.com API error: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "Name.com auth failed (HTTP {status}): {body}"
        )));
    }

    Ok(())
}

/// Mask sensitive fields in Name.com config.
pub fn mask_config(config: &serde_json::Value) -> serde_json::Value {
    let username = config
        .get("username")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let api_token = config
        .get("api_token")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    serde_json::json!({
        "username": username,
        "api_token": mask_token(api_token),
    })
}

fn mask_token(token: &str) -> String {
    if token.len() <= 8 {
        "****".into()
    } else {
        format!("{}...{}", &token[..4], &token[token.len() - 4..])
    }
}

/// Full sync: fetch all domains and DNS records from Name.com.
pub async fn sync(state: &AppState, account: &DnsAccount) -> Result<DnsSyncResult, AppError> {
    let client = &state.http_client;
    let (username, api_token) = basic_auth(&account.config)?;

    // Fetch all domains (paginated)
    let mut all_domains: Vec<NcDomain> = Vec::new();
    let mut page_num = 1;
    loop {
        let resp = client
            .get(format!(
                "{NAMECOM_API_BASE}/domains?perPage=100&page={page_num}"
            ))
            .basic_auth(&username, Some(&api_token))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Name.com API error: {e}")))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "Name.com list domains failed: {body}"
            )));
        }

        let body: NcListDomainsResponse = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Name.com parse error: {e}")))?;

        if let Some(domains) = body.domains {
            all_domains.extend(domains);
        }

        match body.next_page {
            Some(next) if next > page_num => page_num = next,
            _ => break,
        }
    }

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

    for domain in &all_domains {
        seen_zone_ids.push(domain.domain_name.clone());

        // Parse domain expiry
        let domain_expires = domain
            .expire_date
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
        .bind(&domain.domain_name)
        .bind(&domain.domain_name)
        .bind("active")
        .bind(domain_expires)
        .bind(now)
        .fetch_one(&state.db)
        .await?;

        // Fetch DNS records for this domain
        let records_resp = client
            .get(format!(
                "{NAMECOM_API_BASE}/domains/{}/records",
                domain.domain_name
            ))
            .basic_auth(&username, Some(&api_token))
            .send()
            .await;

        let mut seen_record_ids: Vec<String> = Vec::new();

        if let Ok(resp) = records_resp {
            if let Ok(body) = resp.json::<NcListRecordsResponse>().await {
                if let Some(records) = body.records {
                    for rec in &records {
                        let record_id = rec.id.to_string();
                        seen_record_ids.push(record_id.clone());

                        let vps_id =
                            if rec.record_type == "A" || rec.record_type == "AAAA" {
                                ip_to_vps.get(&rec.answer).copied()
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
                        .bind(&rec.fqdn)
                        .bind(&rec.answer)
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
        zones_count: all_domains.len() as i64,
        records_count: total_records,
        records_linked: total_linked,
        records_deleted: total_deleted,
    })
}
