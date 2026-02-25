use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{CloudAccount, CloudSyncResult, IpEntry};
use crate::routes::AppState;

/// Validate AWS credentials by calling DescribeInstances with max_results=5.
pub async fn validate(config: &serde_json::Value) -> Result<(), AppError> {
    let access_key_id = config
        .get("access_key_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing access_key_id in config".into()))?;
    let secret_access_key = config
        .get("secret_access_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing secret_access_key in config".into()))?;
    let regions = parse_regions(config);
    if regions.is_empty() {
        return Err(AppError::BadRequest("At least one region is required".into()));
    }

    let first_region = &regions[0];
    let sdk_config = build_sdk_config(access_key_id, secret_access_key, first_region).await;
    let client = aws_sdk_ec2::Client::new(&sdk_config);

    client
        .describe_instances()
        .max_results(5)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("AWS EC2 auth failed: {e}")))?;

    Ok(())
}

/// Mask sensitive fields in AWS config.
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

/// Full sync: fetch all EC2 instances across configured regions.
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

    // Auto-match or create provider record for AWS
    let provider_id = ensure_provider(state, "AWS").await?;

    let mut instances_found: i64 = 0;
    let mut created: i64 = 0;
    let mut updated: i64 = 0;
    let mut seen_instance_ids: Vec<String> = Vec::new();

    for region in &regions {
        let sdk_config = build_sdk_config(access_key_id, secret_access_key, region).await;
        let client = aws_sdk_ec2::Client::new(&sdk_config);

        // Fetch all instances (exclude terminated)
        let mut next_token: Option<String> = None;
        loop {
            let mut req = client
                .describe_instances()
                .filters(
                    aws_sdk_ec2::types::Filter::builder()
                        .name("instance-state-name")
                        .values("pending")
                        .values("running")
                        .values("stopping")
                        .values("stopped")
                        .build(),
                )
                .max_results(100);

            if let Some(ref token) = next_token {
                req = req.next_token(token);
            }

            let resp = req
                .send()
                .await
                .map_err(|e| AppError::Internal(format!("EC2 DescribeInstances error in {region}: {e}")))?;

            for reservation in resp.reservations() {
                for instance in reservation.instances() {
                    let instance_id = instance.instance_id().unwrap_or_default().to_string();
                    if instance_id.is_empty() {
                        continue;
                    }
                    seen_instance_ids.push(instance_id.clone());
                    instances_found += 1;

                    // Extract Name tag
                    let hostname = instance
                        .tags()
                        .iter()
                        .find(|t| t.key() == Some("Name"))
                        .and_then(|t| t.value())
                        .unwrap_or(&instance_id)
                        .to_string();

                    // IPs
                    let mut ips = Vec::new();
                    if let Some(public_ip) = instance.public_ip_address() {
                        ips.push(IpEntry {
                            ip: public_ip.to_string(),
                            label: "overseas".to_string(),
                        });
                    }
                    if let Some(private_ip) = instance.private_ip_address() {
                        ips.push(IpEntry {
                            ip: private_ip.to_string(),
                            label: "internal".to_string(),
                        });
                    }
                    let ip_json = serde_json::to_value(&ips).unwrap_or_default();

                    // Status mapping
                    let status = match instance.state().and_then(|s| s.name()) {
                        Some(aws_sdk_ec2::types::InstanceStateName::Running) => "active",
                        Some(aws_sdk_ec2::types::InstanceStateName::Stopped) => "suspended",
                        Some(aws_sdk_ec2::types::InstanceStateName::Pending) => "provisioning",
                        Some(aws_sdk_ec2::types::InstanceStateName::Stopping) => "retiring",
                        _ => "active",
                    };

                    // Instance type for cpu/ram hints
                    let instance_type = instance
                        .instance_type()
                        .map(|t| t.as_str().to_string())
                        .unwrap_or_default();

                    // Region → country mapping (approximate)
                    let country = region_to_country(region);

                    // Build extra with cloud metadata
                    let extra = serde_json::json!({
                        "cloud_instance_id": instance_id,
                        "cloud_provider": "aws",
                        "cloud_region": region,
                        "instance_type": instance_type,
                    });

                    // Upsert: check if VPS with this cloud_instance_id exists
                    let existing_vps: Option<(Uuid, String)> = sqlx::query_as(
                        "SELECT id, source FROM vps WHERE extra->>'cloud_instance_id' = $1",
                    )
                    .bind(&instance_id)
                    .fetch_optional(&state.db)
                    .await?;

                    if let Some((vps_id, _)) = existing_vps {
                        // Update existing VPS
                        sqlx::query(
                            r#"UPDATE vps SET
                                hostname = $2,
                                ip_addresses = $3,
                                status = $4,
                                country = $5,
                                provider_id = $6,
                                cloud_account_id = $7,
                                source = 'cloud-sync',
                                extra = extra || $8::jsonb
                            WHERE id = $1"#,
                        )
                        .bind(vps_id)
                        .bind(&hostname)
                        .bind(&ip_json)
                        .bind(status)
                        .bind(country)
                        .bind(provider_id)
                        .bind(account.id)
                        .bind(&extra)
                        .execute(&state.db)
                        .await?;
                        updated += 1;
                    } else {
                        // Create new VPS
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
                                NULL, NULL, NULL, NULL,
                                NULL, 'USD',
                                $5, NULL, NULL,
                                '', '', '{}',
                                true, 9100,
                                $6, '',
                                'cloud-sync', $7
                            )"#,
                        )
                        .bind(&hostname)
                        .bind(provider_id)
                        .bind(&ip_json)
                        .bind(country)
                        .bind(status)
                        .bind(&extra)
                        .bind(account.id)
                        .execute(&state.db)
                        .await?;
                        created += 1;
                    }
                }
            }

            next_token = resp.next_token().map(|s| s.to_string());
            if next_token.is_none() {
                break;
            }
        }
    }

    // Retire stale VPS: those linked to this account but not seen in sync
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

fn parse_regions(config: &serde_json::Value) -> Vec<String> {
    config
        .get("regions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_else(|| vec!["us-east-1".to_string()])
}

async fn build_sdk_config(
    access_key_id: &str,
    secret_access_key: &str,
    region: &str,
) -> aws_config::SdkConfig {
    let creds = aws_credential_types::Credentials::new(
        access_key_id,
        secret_access_key,
        None,
        None,
        "sigma-cloud",
    );
    aws_config::defaults(aws_config::BehaviorVersion::latest())
        .credentials_provider(creds)
        .region(aws_config::Region::new(region.to_string()))
        .load()
        .await
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "****".into()
    } else {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    }
}

/// Ensure a Provider record named "AWS" exists, return its ID.
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
            "INSERT INTO providers (name, country, website, panel_url, api_supported, notes) VALUES ($1, '', 'https://aws.amazon.com', '', true, 'Auto-created by cloud sync') RETURNING id",
        )
        .bind(name)
        .fetch_one(&state.db)
        .await?;
        Ok(id)
    }
}

/// Map AWS region prefix to ISO country code (approximate).
fn region_to_country(region: &str) -> &'static str {
    static REGION_MAP: &[(&str, &str)] = &[
        ("us-", "US"),
        ("ca-", "CA"),
        ("eu-west-1", "IE"),
        ("eu-west-2", "GB"),
        ("eu-west-3", "FR"),
        ("eu-central-1", "DE"),
        ("eu-central-2", "CH"),
        ("eu-south-1", "IT"),
        ("eu-south-2", "ES"),
        ("eu-north-1", "SE"),
        ("ap-northeast-1", "JP"),
        ("ap-northeast-2", "KR"),
        ("ap-northeast-3", "JP"),
        ("ap-southeast-1", "SG"),
        ("ap-southeast-2", "AU"),
        ("ap-southeast-3", "ID"),
        ("ap-south-1", "IN"),
        ("ap-south-2", "IN"),
        ("ap-east-1", "HK"),
        ("sa-east-1", "BR"),
        ("me-south-1", "BH"),
        ("me-central-1", "AE"),
        ("af-south-1", "ZA"),
        ("cn-", "CN"),
        ("il-central-1", "IL"),
    ];

    for (prefix, country) in REGION_MAP {
        if region.starts_with(prefix) {
            return country;
        }
    }
    ""
}
