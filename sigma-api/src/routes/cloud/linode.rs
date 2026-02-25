use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{CloudAccount, CloudSyncResult, IpEntry};
use crate::routes::AppState;

/// Validate Linode (Akamai) credentials by calling GET /v4/profile.
pub async fn validate(
    http_client: &reqwest::Client,
    config: &serde_json::Value,
) -> Result<(), AppError> {
    let api_token = config
        .get("api_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing api_token in config".into()))?;

    let resp = http_client
        .get("https://api.linode.com/v4/profile")
        .bearer_auth(api_token)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Linode API request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "Linode auth failed ({status}): {body}"
        )));
    }

    Ok(())
}

/// Mask sensitive fields in Linode config.
pub fn mask_config(config: &serde_json::Value) -> serde_json::Value {
    let api_token = config
        .get("api_token")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    serde_json::json!({
        "api_token": mask_key(api_token),
    })
}

/// Full sync: fetch all Linodes.
pub async fn sync(state: &AppState, account: &CloudAccount) -> Result<CloudSyncResult, AppError> {
    let api_token = account
        .config
        .get("api_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Internal("Missing api_token in config".into()))?;

    let provider_id = ensure_provider(state, "Linode").await?;

    let mut instances_found: i64 = 0;
    let mut created: i64 = 0;
    let mut updated: i64 = 0;
    let mut seen_instance_ids: Vec<String> = Vec::new();

    let mut page = 1u32;
    loop {
        let resp = state
            .http_client
            .get("https://api.linode.com/v4/linode/instances")
            .bearer_auth(api_token)
            .query(&[("page", page.to_string()), ("page_size", "500".to_string())])
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Linode API error: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "Linode list instances failed ({status}): {body}"
            )));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Linode response parse error: {e}")))?;

        let instances = body["data"].as_array().cloned().unwrap_or_default();
        if instances.is_empty() {
            break;
        }

        for inst in &instances {
            let linode_id = inst["id"].as_u64().unwrap_or(0);
            if linode_id == 0 {
                continue;
            }
            let instance_id = linode_id.to_string();
            seen_instance_ids.push(instance_id.clone());
            instances_found += 1;

            let hostname = inst["label"].as_str().unwrap_or(&instance_id).to_string();

            // IPs
            let mut ips = Vec::new();
            if let Some(ipv4) = inst["ipv4"].as_array() {
                for ip in ipv4 {
                    if let Some(ip_str) = ip.as_str() {
                        let label = if ip_str.starts_with("10.")
                            || ip_str.starts_with("192.168.")
                            || ip_str.starts_with("172.")
                        {
                            "internal"
                        } else {
                            "overseas"
                        };
                        ips.push(IpEntry {
                            ip: ip_str.to_string(),
                            label: label.to_string(),
                        });
                    }
                }
            }
            if let Some(ipv6) = inst["ipv6"].as_str() {
                // Linode returns ipv6 as "xxxx::xxxx/128" — strip CIDR
                let ipv6_clean = ipv6.split('/').next().unwrap_or(ipv6);
                if !ipv6_clean.is_empty() {
                    ips.push(IpEntry {
                        ip: ipv6_clean.to_string(),
                        label: "overseas".to_string(),
                    });
                }
            }
            let ip_json = serde_json::to_value(&ips).unwrap_or_default();

            // Status mapping
            let status = match inst["status"].as_str().unwrap_or("") {
                "running" => "active",
                "offline" | "stopped" => "suspended",
                "provisioning" | "booting" | "rebooting" | "rebuilding" | "migrating" | "resizing" | "restoring" | "cloning" => "provisioning",
                "shutting_down" | "deleting" => "retiring",
                _ => "active",
            };

            let vcpus = inst["specs"]["vcpus"].as_i64().map(|v| v as i16);
            let memory_mb = inst["specs"]["memory"].as_i64().map(|v| v as i32);
            let disk_gb = inst["specs"]["disk"].as_i64().map(|v| (v / 1024) as i32);

            let region = inst["region"].as_str().unwrap_or("").to_string();
            let country = region_to_country(&region);

            let linode_type = inst["type"].as_str().unwrap_or("").to_string();

            let extra = serde_json::json!({
                "cloud_instance_id": instance_id,
                "cloud_provider": "linode",
                "cloud_region": region,
                "linode_type": linode_type,
            });

            // Upsert
            let existing_vps: Option<(Uuid, String)> = sqlx::query_as(
                "SELECT id, source FROM vps WHERE extra->>'cloud_instance_id' = $1",
            )
            .bind(&instance_id)
            .fetch_optional(&state.db)
            .await?;

            if let Some((vps_id, _)) = existing_vps {
                sqlx::query(
                    r#"UPDATE vps SET
                        hostname = $2,
                        ip_addresses = $3,
                        status = $4,
                        country = $5,
                        provider_id = $6,
                        cloud_account_id = $7,
                        cpu_cores = COALESCE($8, cpu_cores),
                        ram_mb = COALESCE($9, ram_mb),
                        disk_gb = COALESCE($10, disk_gb),
                        currency = 'USD',
                        source = 'cloud-sync',
                        extra = extra || $11::jsonb
                    WHERE id = $1"#,
                )
                .bind(vps_id)
                .bind(&hostname)
                .bind(&ip_json)
                .bind(status)
                .bind(country)
                .bind(provider_id)
                .bind(account.id)
                .bind(vcpus)
                .bind(memory_mb)
                .bind(disk_gb)
                .bind(&extra)
                .execute(&state.db)
                .await?;
                updated += 1;
            } else {
                sqlx::query(
                    r#"INSERT INTO vps (
                        hostname, alias, provider_id,
                        ip_addresses, ssh_port,
                        country, city, dc_name,
                        cpu_cores, ram_mb, disk_gb, bandwidth_tb,
                        cost_monthly, currency,
                        status, purchase_date, expire_date,
                        purpose, vpn_protocol, tags,
                        monitoring_enabled, node_exporter_port,
                        extra, notes,
                        source, cloud_account_id
                    ) VALUES (
                        $1, '', $2,
                        $3, 22,
                        $4, '', '',
                        $5, $6, $7, NULL,
                        NULL, 'USD',
                        $8, NULL, NULL,
                        '', '', '{}',
                        true, 9100,
                        $9, '',
                        'cloud-sync', $10
                    )"#,
                )
                .bind(&hostname)
                .bind(provider_id)
                .bind(&ip_json)
                .bind(country)
                .bind(vcpus)
                .bind(memory_mb)
                .bind(disk_gb)
                .bind(status)
                .bind(&extra)
                .bind(account.id)
                .execute(&state.db)
                .await?;
                created += 1;
            }
        }

        let total_pages = body["pages"].as_i64().unwrap_or(1);
        if (page as i64) >= total_pages {
            break;
        }
        page += 1;
    }

    // Retire stale VPS
    let retired = if !seen_instance_ids.is_empty() {
        let result = sqlx::query(
            r#"UPDATE vps SET status = 'retired', monitoring_enabled = false
               WHERE cloud_account_id = $1
                 AND source = 'cloud-sync'
                 AND extra->>'cloud_instance_id' IS NOT NULL
                 AND extra->>'cloud_instance_id' != ALL($2)
                 AND status != 'retired'"#,
        )
        .bind(account.id)
        .bind(&seen_instance_ids)
        .execute(&state.db)
        .await?;
        result.rows_affected() as i64
    } else {
        0
    };

    Ok(CloudSyncResult {
        instances_found,
        created,
        updated,
        retired,
    })
}

// ─── Helpers ──────────────────────────────────────────────

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "****".into()
    } else {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    }
}

async fn ensure_provider(state: &AppState, name: &str) -> Result<Uuid, AppError> {
    let existing: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM providers WHERE LOWER(name) = LOWER($1) LIMIT 1")
            .bind(name)
            .fetch_optional(&state.db)
            .await?;

    if let Some((id,)) = existing {
        Ok(id)
    } else {
        let (id,): (Uuid,) = sqlx::query_as(
            "INSERT INTO providers (name, country, website, panel_url, api_supported, notes) VALUES ($1, '', 'https://www.linode.com', '', true, 'Auto-created by cloud sync') RETURNING id",
        )
        .bind(name)
        .fetch_one(&state.db)
        .await?;
        Ok(id)
    }
}

/// Map Linode region to ISO country code.
fn region_to_country(region: &str) -> &'static str {
    match region {
        "us-east" | "us-central" | "us-west" | "us-southeast" | "us-iad" | "us-ord" | "us-lax" | "us-mia" => "US",
        "ca-central" => "CA",
        "eu-west" | "eu-central" | "fr-par" => "FR",
        "eu-west-1" | "gb-lon" => "GB",
        "de-fra-1" | "de-fra-2" => "DE",
        "nl-ams-1" => "NL",
        "it-mil-1" => "IT",
        "se-sto-1" => "SE",
        "es-mad-1" => "ES",
        "ap-west" | "in-maa" | "in-bom-1" => "IN",
        "ap-southeast" | "sg-sin-1" => "SG",
        "ap-south" | "ap-southeast-2" | "au-mel" => "AU",
        "ap-northeast" | "jp-osa" | "jp-tyo-3" => "JP",
        "id-cgk" => "ID",
        "br-gru" => "BR",
        "za-jnb-1" => "ZA",
        _ => "",
    }
}
