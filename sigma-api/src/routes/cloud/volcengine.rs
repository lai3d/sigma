use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{CloudAccount, CloudSyncResult, IpEntry};
use crate::routes::AppState;

type HmacSha256 = Hmac<Sha256>;

const API_HOST: &str = "open.volcengineapi.com";
const API_ENDPOINT: &str = "https://open.volcengineapi.com/";
const SERVICE: &str = "ecs";
const API_VERSION: &str = "2020-04-01";

/// Validate Volcengine credentials by calling DescribeRegions.
pub async fn validate(config: &serde_json::Value) -> Result<(), AppError> {
    let access_key_id = config
        .get("access_key_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing access_key_id in config".into()))?;
    let secret_access_key = config
        .get("secret_access_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing secret_access_key in config".into()))?;

    let client = reqwest::Client::new();
    let _resp = call_api(
        &client,
        access_key_id,
        secret_access_key,
        "cn-beijing",
        "DescribeRegions",
        &[],
    )
    .await?;

    Ok(())
}

/// Mask sensitive fields in Volcengine config.
pub fn mask_config(config: &serde_json::Value) -> serde_json::Value {
    let access_key_id = config
        .get("access_key_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let regions = parse_regions(config);

    serde_json::json!({
        "access_key_id": mask_key(access_key_id),
        "secret_access_key": "****",
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
    let secret_access_key = account
        .config
        .get("secret_access_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Internal("Missing secret_access_key in config".into()))?;
    let regions = parse_regions(&account.config);

    let provider_id = ensure_provider(state, "Volcengine").await?;
    let client = reqwest::Client::new();

    let mut instances_found: i64 = 0;
    let mut created: i64 = 0;
    let mut updated: i64 = 0;
    let mut seen_instance_ids: Vec<String> = Vec::new();

    for region in &regions {
        let mut next_token: Option<String> = None;
        loop {
            let mut extra_params: Vec<(&str, &str)> = vec![("MaxResults", "100")];
            let token_str;
            if let Some(ref token) = next_token {
                token_str = token.clone();
                extra_params.push(("NextToken", &token_str));
            }

            let resp = call_api(
                &client,
                access_key_id,
                secret_access_key,
                region,
                "DescribeInstances",
                &extra_params,
            )
            .await?;

            let instances = resp["Result"]["Instances"]
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

                let hostname = inst["InstanceName"]
                    .as_str()
                    .or_else(|| inst["Hostname"].as_str())
                    .unwrap_or(&instance_id)
                    .to_string();

                // IPs
                let mut ips = Vec::new();
                // EIP (public)
                if let Some(eip) = inst["EipAddress"]["IpAddress"].as_str() {
                    if !eip.is_empty() {
                        ips.push(IpEntry {
                            ip: eip.to_string(),
                            label: "overseas".to_string(),
                        });
                    }
                }
                // Private IPs from NetworkInterfaces
                if let Some(nics) = inst["NetworkInterfaces"].as_array() {
                    for nic in nics {
                        if let Some(ip) = nic["PrimaryIpAddress"].as_str() {
                            if !ip.is_empty() {
                                ips.push(IpEntry {
                                    ip: ip.to_string(),
                                    label: "internal".to_string(),
                                });
                            }
                        }
                        // IPv6
                        if let Some(v6_addrs) = nic["Ipv6Addresses"].as_array() {
                            for v6 in v6_addrs {
                                if let Some(ip) = v6.as_str() {
                                    if !ip.is_empty() {
                                        ips.push(IpEntry {
                                            ip: ip.to_string(),
                                            label: "internal".to_string(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                let ip_json = serde_json::to_value(&ips).unwrap_or_default();

                // Status mapping
                let status = match inst["Status"].as_str().unwrap_or("") {
                    "RUNNING" => "active",
                    "STOPPED" => "suspended",
                    "CREATING" | "STARTING" | "REBOOTING" | "REBUILDING" => "provisioning",
                    "STOPPING" | "DELETING" => "retiring",
                    "ERROR" => "suspended",
                    _ => "active",
                };

                let vcpus = inst["Cpus"].as_i64().map(|v| v as i16);
                let memory_mb = inst["MemorySize"].as_i64().map(|v| v as i32);
                let instance_type = inst["InstanceTypeId"].as_str().unwrap_or("").to_string();
                let zone_id = inst["ZoneId"].as_str().unwrap_or("").to_string();
                let country = region_to_country(region);

                let extra = serde_json::json!({
                    "cloud_instance_id": instance_id,
                    "cloud_provider": "volcengine",
                    "cloud_region": region,
                    "zone_id": zone_id,
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
                    .bind(vcpus)
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
                            NULL, 'CNY',
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
                    .bind(vcpus)
                    .bind(memory_mb)
                    .bind(status)
                    .bind(&extra)
                    .bind(account.id)
                    .execute(&state.db)
                    .await?;
                    created += 1;
                }
            }

            // Pagination
            let resp_next = resp["Result"]["NextToken"].as_str().unwrap_or("");
            if resp_next.is_empty() {
                break;
            }
            next_token = Some(resp_next.to_string());
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

// ─── Volcengine API V4 signing ──────────────────────────

async fn call_api(
    client: &reqwest::Client,
    access_key_id: &str,
    secret_access_key: &str,
    region: &str,
    action: &str,
    extra_params: &[(&str, &str)],
) -> Result<serde_json::Value, AppError> {
    let now = Utc::now();
    let x_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let short_date = now.format("%Y%m%d").to_string();

    // Build query parameters
    let mut params: Vec<(&str, &str)> = vec![
        ("Action", action),
        ("Version", API_VERSION),
    ];
    for (k, v) in extra_params {
        params.push((k, v));
    }
    params.sort_by(|a, b| a.0.cmp(b.0));

    let canonical_qs: String = params
        .iter()
        .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    // Request body is empty for GET
    let body_hash = hex_sha256(b"");

    // Canonical headers (sorted)
    let canonical_headers = format!(
        "content-type:application/x-www-form-urlencoded\nhost:{}\nx-content-sha256:{}\nx-date:{}\n",
        API_HOST, body_hash, x_date
    );
    let signed_headers = "content-type;host;x-content-sha256;x-date";

    // Canonical request
    let canonical_request = format!(
        "GET\n/\n{}\n{}\n{}\n{}",
        canonical_qs, canonical_headers, signed_headers, body_hash
    );

    // Credential scope
    let credential_scope = format!("{}/{}/{}/request", short_date, region, SERVICE);

    // String to sign
    let string_to_sign = format!(
        "HMAC-SHA256\n{}\n{}\n{}",
        x_date,
        credential_scope,
        hex_sha256(canonical_request.as_bytes())
    );

    // Derive signing key
    let k_date = hmac_sha256(secret_access_key.as_bytes(), short_date.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, SERVICE.as_bytes());
    let k_signing = hmac_sha256(&k_service, b"request");

    // Signature
    let signature = hex::encode(hmac_sha256(&k_signing, string_to_sign.as_bytes()));

    let authorization = format!(
        "HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
        access_key_id, credential_scope, signed_headers, signature
    );

    let url = format!("{}?{}", API_ENDPOINT, canonical_qs);

    let resp = client
        .get(&url)
        .header("Host", API_HOST)
        .header("X-Date", &x_date)
        .header("X-Content-Sha256", &body_hash)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Authorization", &authorization)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Volcengine API request failed: {e}")))?;

    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("Volcengine API response parse error: {e}")))?;

    // Check for API error in ResponseMetadata
    if let Some(err) = body["ResponseMetadata"]["Error"].as_object() {
        let code = err
            .get("Code")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let message = err
            .get("Message")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        return Err(AppError::BadRequest(format!(
            "Volcengine API error ({code}): {message}"
        )));
    }

    if !status.is_success() {
        return Err(AppError::Internal(format!(
            "Volcengine API HTTP error: {status}"
        )));
    }

    Ok(body)
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn hex_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
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
        .unwrap_or_else(|| vec!["cn-beijing".to_string()])
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "****".into()
    } else {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    }
}

/// Ensure a Provider record for Volcengine exists, return its ID.
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
            "INSERT INTO providers (name, country, website, panel_url, api_supported, notes) VALUES ($1, 'CN', 'https://www.volcengine.com', '', true, 'Auto-created by cloud sync') RETURNING id",
        )
        .bind(name)
        .fetch_one(&state.db)
        .await?;
        Ok(id)
    }
}

/// Map Volcengine region to ISO country code.
fn region_to_country(region: &str) -> &'static str {
    match region {
        "cn-beijing" | "cn-shanghai" | "cn-guangzhou" => "CN",
        "cn-hongkong" => "HK",
        "ap-southeast-1" => "MY",
        "ap-southeast-3" => "ID",
        _ if region.starts_with("cn-") => "CN",
        _ => "",
    }
}
