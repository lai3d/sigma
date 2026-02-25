use chrono::Utc;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{CloudAccount, CloudSyncResult, IpEntry};
use crate::routes::AppState;

type HmacSha1 = Hmac<Sha1>;

/// Validate Alibaba Cloud credentials by calling DescribeRegions.
pub async fn validate(config: &serde_json::Value) -> Result<(), AppError> {
    let access_key_id = config
        .get("access_key_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing access_key_id in config".into()))?;
    let access_key_secret = config
        .get("access_key_secret")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing access_key_secret in config".into()))?;

    let client = reqwest::Client::new();
    let _resp = call_ecs_api(
        &client,
        access_key_id,
        access_key_secret,
        "cn-hangzhou",
        "DescribeRegions",
        &[],
    )
    .await?;

    Ok(())
}

/// Mask sensitive fields in Alibaba config.
pub fn mask_config(config: &serde_json::Value) -> serde_json::Value {
    let access_key_id = config
        .get("access_key_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let regions = parse_regions(config);

    serde_json::json!({
        "access_key_id": mask_key(access_key_id),
        "access_key_secret": "****",
        "regions": regions,
    })
}

/// Full sync: fetch all ECS instances across configured regions.
pub async fn sync(state: &AppState, account: &CloudAccount) -> Result<CloudSyncResult, AppError> {
    let access_key_id = account
        .config
        .get("access_key_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Internal("Missing access_key_id in config".into()))?;
    let access_key_secret = account
        .config
        .get("access_key_secret")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Internal("Missing access_key_secret in config".into()))?;
    let regions = parse_regions(&account.config);

    let provider_id = ensure_provider(state, "Alibaba Cloud").await?;
    let client = reqwest::Client::new();

    let mut instances_found: i64 = 0;
    let mut created: i64 = 0;
    let mut updated: i64 = 0;
    let mut seen_instance_ids: Vec<String> = Vec::new();

    for region in &regions {
        let mut page_number = 1;
        loop {
            let resp = call_ecs_api(
                &client,
                access_key_id,
                access_key_secret,
                region,
                "DescribeInstances",
                &[
                    ("PageSize", "50"),
                    ("PageNumber", &page_number.to_string()),
                ],
            )
            .await?;

            let instances = resp["Instances"]["Instance"]
                .as_array()
                .cloned()
                .unwrap_or_default();

            if instances.is_empty() {
                break;
            }

            for inst in &instances {
                let instance_id = inst["InstanceId"].as_str().unwrap_or_default().to_string();
                if instance_id.is_empty() {
                    continue;
                }
                seen_instance_ids.push(instance_id.clone());
                instances_found += 1;

                let hostname = inst["HostName"]
                    .as_str()
                    .unwrap_or(&instance_id)
                    .to_string();

                // IPs
                let mut ips = Vec::new();
                if let Some(public_ips) = inst["PublicIpAddress"]["IpAddress"].as_array() {
                    for ip in public_ips {
                        if let Some(ip_str) = ip.as_str() {
                            ips.push(IpEntry {
                                ip: ip_str.to_string(),
                                label: "overseas".to_string(),
                            });
                        }
                    }
                }
                // EIP
                if let Some(eip) = inst["EipAddress"]["IpAddress"].as_str() {
                    if !eip.is_empty() && !ips.iter().any(|e| e.ip == eip) {
                        ips.push(IpEntry {
                            ip: eip.to_string(),
                            label: "overseas".to_string(),
                        });
                    }
                }
                if let Some(private_ips) = inst["VpcAttributes"]["PrivateIpAddress"]["IpAddress"].as_array() {
                    for ip in private_ips {
                        if let Some(ip_str) = ip.as_str() {
                            ips.push(IpEntry {
                                ip: ip_str.to_string(),
                                label: "internal".to_string(),
                            });
                        }
                    }
                }
                let ip_json = serde_json::to_value(&ips).unwrap_or_default();

                // Status mapping
                let status = match inst["Status"].as_str().unwrap_or("") {
                    "Running" => "active",
                    "Stopped" => "suspended",
                    "Starting" | "Pending" => "provisioning",
                    "Stopping" => "retiring",
                    _ => "active",
                };

                let cpu = inst["Cpu"].as_i64().map(|v| v as i16);
                let memory_mb = inst["Memory"].as_i64().map(|v| v as i32);
                let instance_type = inst["InstanceType"].as_str().unwrap_or("").to_string();
                let country = region_to_country(region);

                let extra = serde_json::json!({
                    "cloud_instance_id": instance_id,
                    "cloud_provider": "alibaba",
                    "cloud_region": region,
                    "instance_type": instance_type,
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
                            source = 'cloud-sync',
                            extra = extra || $10::jsonb
                        WHERE id = $1"#,
                    )
                    .bind(vps_id)
                    .bind(&hostname)
                    .bind(&ip_json)
                    .bind(status)
                    .bind(country)
                    .bind(provider_id)
                    .bind(account.id)
                    .bind(cpu)
                    .bind(memory_mb)
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
                            $5, $6, NULL, NULL,
                            NULL, 'USD',
                            $7, NULL, NULL,
                            '', '', '{}',
                            true, 9100,
                            $8, '',
                            'cloud-sync', $9
                        )"#,
                    )
                    .bind(&hostname)
                    .bind(provider_id)
                    .bind(&ip_json)
                    .bind(country)
                    .bind(cpu)
                    .bind(memory_mb)
                    .bind(status)
                    .bind(&extra)
                    .bind(account.id)
                    .execute(&state.db)
                    .await?;
                    created += 1;
                }
            }

            let total_count = resp["TotalCount"].as_i64().unwrap_or(0);
            if (page_number * 50) as i64 >= total_count {
                break;
            }
            page_number += 1;
        }
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

// ─── Alibaba Cloud API signing ──────────────────────────

async fn call_ecs_api(
    client: &reqwest::Client,
    access_key_id: &str,
    access_key_secret: &str,
    region: &str,
    action: &str,
    extra_params: &[(&str, &str)],
) -> Result<serde_json::Value, AppError> {
    let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let nonce = Uuid::new_v4().to_string();

    let mut params: Vec<(&str, String)> = vec![
        ("Format", "JSON".to_string()),
        ("Version", "2014-05-26".to_string()),
        ("AccessKeyId", access_key_id.to_string()),
        ("SignatureMethod", "HMAC-SHA1".to_string()),
        ("Timestamp", timestamp),
        ("SignatureVersion", "1.0".to_string()),
        ("SignatureNonce", nonce),
        ("Action", action.to_string()),
        ("RegionId", region.to_string()),
    ];

    for (k, v) in extra_params {
        params.push((k, v.to_string()));
    }

    params.sort_by(|a, b| a.0.cmp(b.0));

    // Build canonicalized query string
    let canonical_qs: String = params
        .iter()
        .map(|(k, v)| {
            format!(
                "{}={}",
                percent_encode(k),
                percent_encode(v)
            )
        })
        .collect::<Vec<_>>()
        .join("&");

    // Build string to sign
    let string_to_sign = format!(
        "GET&{}&{}",
        percent_encode("/"),
        percent_encode(&canonical_qs)
    );

    // HMAC-SHA1 signing
    let signing_key = format!("{}&", access_key_secret);
    let mut mac = HmacSha1::new_from_slice(signing_key.as_bytes())
        .map_err(|e| AppError::Internal(format!("HMAC init error: {e}")))?;
    mac.update(string_to_sign.as_bytes());
    let signature = base64_encode(&mac.finalize().into_bytes());

    let url = format!(
        "https://ecs.aliyuncs.com/?{}&Signature={}",
        canonical_qs,
        percent_encode(&signature)
    );

    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Alibaba API request failed: {e}")))?;

    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("Alibaba API response parse error: {e}")))?;

    if !status.is_success() {
        let code = body["Code"].as_str().unwrap_or("Unknown");
        let message = body["Message"].as_str().unwrap_or("Unknown error");
        return Err(AppError::BadRequest(format!(
            "Alibaba API error ({code}): {message}"
        )));
    }

    Ok(body)
}

fn percent_encode(input: &str) -> String {
    let mut result = String::with_capacity(input.len() * 2);
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            _ => {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    result
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

// ─── Helpers ──────────────────────────────────────────────

fn parse_regions(config: &serde_json::Value) -> Vec<String> {
    config
        .get("regions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_else(|| vec!["cn-hangzhou".to_string()])
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "****".into()
    } else {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    }
}

/// Ensure a Provider record for Alibaba Cloud exists, return its ID.
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
            "INSERT INTO providers (name, country, website, panel_url, api_supported, notes) VALUES ($1, '', 'https://www.alibabacloud.com', '', true, 'Auto-created by cloud sync') RETURNING id",
        )
        .bind(name)
        .fetch_one(&state.db)
        .await?;
        Ok(id)
    }
}

/// Map Alibaba Cloud region to ISO country code (approximate).
fn region_to_country(region: &str) -> &'static str {
    static REGION_MAP: &[(&str, &str)] = &[
        ("cn-", "CN"),
        ("us-east-1", "US"),
        ("us-west-1", "US"),
        ("ap-southeast-1", "SG"),
        ("ap-southeast-2", "AU"),
        ("ap-southeast-3", "MY"),
        ("ap-southeast-5", "ID"),
        ("ap-southeast-6", "PH"),
        ("ap-southeast-7", "TH"),
        ("ap-northeast-1", "JP"),
        ("ap-northeast-2", "KR"),
        ("ap-south-1", "IN"),
        ("eu-central-1", "DE"),
        ("eu-west-1", "GB"),
        ("me-east-1", "AE"),
        ("me-central-1", "SA"),
    ];

    for (prefix, country) in REGION_MAP {
        if region.starts_with(prefix) {
            return country;
        }
    }
    ""
}
