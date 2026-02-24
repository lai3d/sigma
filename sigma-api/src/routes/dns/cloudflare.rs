use chrono::{DateTime, Utc};
use serde::Deserialize;
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
        if let Ok(ips) = serde_json::from_value::<Vec<IpEntry>>(ips_json.clone()) {
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
            r#"INSERT INTO dns_zones (account_id, zone_id, zone_name, status, synced_at)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (account_id, zone_id)
               DO UPDATE SET zone_name = $3, status = $4, synced_at = $5, updated_at = now()
               RETURNING id"#,
        )
        .bind(account.id)
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
                    .execute(&state.db)
                    .await?;

                    total_records += 1;
                }
            }
        }

        // Delete stale records for this zone
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

        // Best-effort: fetch cert expiry
        let cert_expires = fetch_cert_expiry(client, token, &zone.id).await;
        if let Some(expires) = cert_expires {
            let _ =
                sqlx::query("UPDATE dns_zones SET cert_expires_at = $2 WHERE id = $1")
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
                "UPDATE dns_zones SET domain_expires_at = $2 WHERE id = $1",
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
            "DELETE FROM dns_zones WHERE account_id = $1 AND zone_id != ALL($2)",
        )
        .bind(account.id)
        .bind(&seen_zone_ids)
        .execute(&state.db)
        .await?;
        total_deleted += deleted.rows_affected() as i64;
    }

    Ok(DnsSyncResult {
        zones_count: all_zones.len() as i64,
        records_count: total_records,
        records_linked: total_linked,
        records_deleted: total_deleted,
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
