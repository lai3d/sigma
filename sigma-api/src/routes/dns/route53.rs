use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{DnsAccount, DnsSyncResult, IpEntry};
use crate::routes::AppState;

/// Validate AWS Route 53 credentials by listing hosted zones.
pub async fn validate(config: &serde_json::Value) -> Result<(), AppError> {
    let access_key_id = config
        .get("access_key_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing access_key_id in config".into()))?;
    let secret_access_key = config
        .get("secret_access_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing secret_access_key in config".into()))?;
    let region = config
        .get("region")
        .and_then(|v| v.as_str())
        .unwrap_or("us-east-1");

    let creds = aws_credential_types::Credentials::new(
        access_key_id,
        secret_access_key,
        None,
        None,
        "sigma-dns",
    );
    let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .credentials_provider(creds)
        .region(aws_config::Region::new(region.to_string()))
        .load()
        .await;

    let client = aws_sdk_route53::Client::new(&sdk_config);

    client
        .list_hosted_zones()
        .max_items(1)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Route 53 auth failed: {e}")))?;

    Ok(())
}

/// Mask sensitive fields in Route 53 config.
pub fn mask_config(config: &serde_json::Value) -> serde_json::Value {
    let access_key_id = config
        .get("access_key_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let region = config
        .get("region")
        .and_then(|v| v.as_str())
        .unwrap_or("us-east-1");

    serde_json::json!({
        "access_key_id": mask_key(access_key_id),
        "secret_access_key": "****",
        "region": region,
    })
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "****".into()
    } else {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    }
}

/// Full sync: fetch all hosted zones and record sets from Route 53.
pub async fn sync(state: &AppState, account: &DnsAccount) -> Result<DnsSyncResult, AppError> {
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
    let region = account
        .config
        .get("region")
        .and_then(|v| v.as_str())
        .unwrap_or("us-east-1");

    let creds = aws_credential_types::Credentials::new(
        access_key_id,
        secret_access_key,
        None,
        None,
        "sigma-dns",
    );
    let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .credentials_provider(creds)
        .region(aws_config::Region::new(region.to_string()))
        .load()
        .await;

    let client = aws_sdk_route53::Client::new(&sdk_config);

    // Fetch all hosted zones
    let mut all_zones = Vec::new();
    let mut marker: Option<String> = None;
    loop {
        let mut req = client.list_hosted_zones().max_items(100);
        if let Some(ref m) = marker {
            req = req.marker(m);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Route 53 list zones error: {e}")))?;

        all_zones.extend(resp.hosted_zones);

        if resp.is_truncated {
            marker = resp.next_marker;
        } else {
            break;
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

    for zone in &all_zones {
        let zone_id = zone.id.as_str();
        // Route 53 zone IDs are like "/hostedzone/Z123ABC" — extract the ID part
        let zone_id_clean = zone_id.trim_start_matches("/hostedzone/");
        let zone_name = zone.name.as_str().trim_end_matches('.');

        seen_zone_ids.push(zone_id_clean.to_string());

        // Upsert zone (no expiry info from Route 53)
        let zone_uuid: (Uuid,) = sqlx::query_as(
            r#"INSERT INTO dns_zones (account_id, zone_id, zone_name, status, synced_at)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (account_id, zone_id)
               DO UPDATE SET zone_name = $3, status = $4, synced_at = $5, updated_at = now()
               RETURNING id"#,
        )
        .bind(account.id)
        .bind(zone_id_clean)
        .bind(zone_name)
        .bind("active")
        .bind(now)
        .fetch_one(&state.db)
        .await?;

        // Fetch all record sets for this zone
        let mut seen_record_ids: Vec<String> = Vec::new();
        let mut start_record_name: Option<String> = None;
        let mut start_record_type: Option<aws_sdk_route53::types::RrType> = None;

        loop {
            let mut req = client
                .list_resource_record_sets()
                .hosted_zone_id(zone_id_clean)
                .max_items(300);
            if let Some(ref name) = start_record_name {
                req = req.start_record_name(name);
            }
            if let Some(ref rtype) = start_record_type {
                req = req.start_record_type(rtype.clone());
            }

            let resp = req.send().await;

            if let Ok(resp) = resp {
                for rrset in &resp.resource_record_sets {
                    let record_type = rrset.r#type.as_str().to_string();
                    let name = rrset.name.as_str().trim_end_matches('.');

                    // Each record set can have multiple values
                    if let Some(ref records) = rrset.resource_records {
                        for (i, rr) in records.iter().enumerate() {
                            let content = rr.value.as_str();
                            // Use name+type+index as record_id since Route 53 doesn't have individual record IDs
                            let record_id = format!("{name}:{record_type}:{i}");
                            seen_record_ids.push(record_id.clone());

                            let vps_id =
                                if record_type == "A" || record_type == "AAAA" {
                                    ip_to_vps.get(content).copied()
                                } else {
                                    None
                                };

                            if vps_id.is_some() {
                                total_linked += 1;
                            }

                            let ttl = rrset.ttl.unwrap_or(300) as i32;

                            sqlx::query(
                                r#"INSERT INTO dns_records (zone_uuid, record_id, record_type, name, content, ttl, extra, vps_id, synced_at)
                                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                                   ON CONFLICT (zone_uuid, record_id)
                                   DO UPDATE SET record_type = $3, name = $4, content = $5, ttl = $6, extra = $7, vps_id = $8, synced_at = $9, updated_at = now()"#,
                            )
                            .bind(zone_uuid.0)
                            .bind(&record_id)
                            .bind(&record_type)
                            .bind(name)
                            .bind(content)
                            .bind(ttl)
                            .bind(serde_json::json!({}))
                            .bind(vps_id)
                            .bind(now)
                            .execute(&state.db)
                            .await?;

                            total_records += 1;
                        }
                    }
                }

                if resp.is_truncated {
                    start_record_name = resp.next_record_name;
                    start_record_type = resp.next_record_type;
                } else {
                    break;
                }
            } else {
                break;
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
        zones_count: all_zones.len() as i64,
        records_count: total_records,
        records_linked: total_linked,
        records_deleted: total_deleted,
    })
}
