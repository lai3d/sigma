use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{CloudAccount, CloudSyncResult, IpEntry};
use crate::routes::AppState;

/// Validate DigitalOcean credentials by calling GET /v2/account.
pub async fn validate(
    http_client: &reqwest::Client,
    config: &serde_json::Value,
) -> Result<(), AppError> {
    let api_token = config
        .get("api_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing api_token in config".into()))?;

    let resp = http_client
        .get("https://api.digitalocean.com/v2/account")
        .bearer_auth(api_token)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("DigitalOcean API request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "DigitalOcean auth failed ({status}): {body}"
        )));
    }

    Ok(())
}

/// Mask sensitive fields in DigitalOcean config.
pub fn mask_config(config: &serde_json::Value) -> serde_json::Value {
    let api_token = config
        .get("api_token")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    serde_json::json!({
        "api_token": mask_key(api_token),
    })
}

/// Full sync: fetch all Droplets from DigitalOcean.
pub async fn sync(state: &AppState, account: &CloudAccount) -> Result<CloudSyncResult, AppError> {
    let api_token = account
        .config
        .get("api_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Internal("Missing api_token in config".into()))?;

    let provider_id = ensure_provider(state, "DigitalOcean").await?;

    let mut instances_found: i64 = 0;
    let mut created: i64 = 0;
    let mut updated: i64 = 0;
    let mut seen_instance_ids: Vec<String> = Vec::new();

    // Paginate through all droplets
    let mut page = 1u32;
    loop {
        let resp = state
            .http_client
            .get("https://api.digitalocean.com/v2/droplets")
            .bearer_auth(api_token)
            .query(&[("page", page.to_string()), ("per_page", "200".to_string())])
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("DigitalOcean API error: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "DigitalOcean list droplets failed ({status}): {body}"
            )));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("DigitalOcean response parse error: {e}")))?;

        let droplets = body["droplets"].as_array().cloned().unwrap_or_default();
        if droplets.is_empty() {
            break;
        }

        for droplet in &droplets {
            let droplet_id = droplet["id"].as_u64().unwrap_or(0);
            if droplet_id == 0 {
                continue;
            }
            let instance_id = droplet_id.to_string();
            seen_instance_ids.push(instance_id.clone());
            instances_found += 1;

            let hostname = droplet["name"].as_str().unwrap_or(&instance_id).to_string();

            // IPs from networks
            let mut ips = Vec::new();
            if let Some(networks) = droplet["networks"].as_object() {
                if let Some(v4) = networks.get("v4").and_then(|v| v.as_array()) {
                    for net in v4 {
                        if let Some(ip) = net["ip_address"].as_str() {
                            let net_type = net["type"].as_str().unwrap_or("");
                            let label = if net_type == "private" {
                                "internal"
                            } else {
                                "overseas"
                            };
                            ips.push(IpEntry {
                                ip: ip.to_string(),
                                label: label.to_string(),
                            });
                        }
                    }
                }
                if let Some(v6) = networks.get("v6").and_then(|v| v.as_array()) {
                    for net in v6 {
                        if let Some(ip) = net["ip_address"].as_str() {
                            let net_type = net["type"].as_str().unwrap_or("");
                            let label = if net_type == "private" {
                                "internal"
                            } else {
                                "overseas"
                            };
                            ips.push(IpEntry {
                                ip: ip.to_string(),
                                label: label.to_string(),
                            });
                        }
                    }
                }
            }
            let ip_json = serde_json::to_value(&ips).unwrap_or_default();

            // Status mapping
            let status = match droplet["status"].as_str().unwrap_or("") {
                "active" => "active",
                "off" => "suspended",
                "new" => "provisioning",
                "archive" => "retired",
                _ => "active",
            };

            let vcpus = droplet["vcpus"].as_i64().map(|v| v as i16);
            let memory_mb = droplet["memory"].as_i64().map(|v| v as i32);
            let disk_gb = droplet["disk"].as_i64().map(|v| v as i32);

            // Region → country
            let region_slug = droplet["region"]["slug"].as_str().unwrap_or("");
            let country = region_to_country(region_slug);

            // Monthly cost
            let cost_monthly = droplet["size"]["price_monthly"]
                .as_f64()
                .and_then(rust_decimal::Decimal::from_f64_retain);

            let size_slug = droplet["size_slug"].as_str().unwrap_or("").to_string();

            let extra = serde_json::json!({
                "cloud_instance_id": instance_id,
                "cloud_provider": "digitalocean",
                "cloud_region": region_slug,
                "size_slug": size_slug,
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
                        cost_monthly = COALESCE($11, cost_monthly),
                        currency = 'USD',
                        source = 'cloud-sync',
                        extra = extra || $12::jsonb
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
                .bind(cost_monthly)
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
                        $8, 'USD',
                        $9, NULL, NULL,
                        '', '', '{}',
                        true, 9100,
                        $10, '',
                        'cloud-sync', $11
                    )"#,
                )
                .bind(&hostname)
                .bind(provider_id)
                .bind(&ip_json)
                .bind(country)
                .bind(vcpus)
                .bind(memory_mb)
                .bind(disk_gb)
                .bind(cost_monthly)
                .bind(status)
                .bind(&extra)
                .bind(account.id)
                .execute(&state.db)
                .await?;
                created += 1;
            }
        }

        // Check pagination
        let total = body["meta"]["total"].as_i64().unwrap_or(0);
        if (page as i64 * 200) >= total {
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

/// Ensure a Provider record for DigitalOcean exists, return its ID.
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
            "INSERT INTO providers (name, country, website, panel_url, api_supported, notes) VALUES ($1, '', 'https://www.digitalocean.com', '', true, 'Auto-created by cloud sync') RETURNING id",
        )
        .bind(name)
        .fetch_one(&state.db)
        .await?;
        Ok(id)
    }
}

/// Map DigitalOcean region slug to ISO country code.
fn region_to_country(region: &str) -> &'static str {
    match region {
        "nyc1" | "nyc2" | "nyc3" | "sfo1" | "sfo2" | "sfo3" => "US",
        "tor1" => "CA",
        "lon1" => "GB",
        "ams2" | "ams3" => "NL",
        "fra1" => "DE",
        "sgp1" => "SG",
        "blr1" => "IN",
        "syd1" => "AU",
        _ => "",
    }
}
