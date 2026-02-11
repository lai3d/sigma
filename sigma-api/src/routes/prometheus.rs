use axum::{extract::State, routing::get, Json, Router};
use std::collections::HashMap;

use crate::errors::AppError;
use crate::models::{IpEntry, PrometheusTarget};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/prometheus/targets", get(targets))
}

/// Output Prometheus file_sd compatible JSON.
/// Only includes VPS with monitoring_enabled = true and status in (active, provisioning).
async fn targets(State(state): State<AppState>) -> Result<Json<Vec<PrometheusTarget>>, AppError> {
    let rows = sqlx::query_as::<_, VpsTargetRow>(
        r#"SELECT
            v.hostname, v.alias, v.ip_addresses, v.node_exporter_port,
            v.country, v.city, v.dc_name, v.status, v.purpose, v.vpn_protocol,
            v.tags, v.expire_date,
            COALESCE(p.name, '') as provider_name
           FROM vps v
           LEFT JOIN providers p ON p.id = v.provider_id
           WHERE v.monitoring_enabled = true
             AND v.status IN ('active', 'provisioning')
           ORDER BY v.hostname"#,
    )
    .fetch_all(&state.db)
    .await?;

    let targets: Vec<PrometheusTarget> = rows
        .into_iter()
        .filter_map(|row| {
            // Use the first IP address as the scrape target
            let entries = row.ip_addresses.0;
            let first_ip = entries.first()?;
            let target = format!("{}:{}", first_ip.ip, row.node_exporter_port);

            let mut labels = HashMap::new();
            labels.insert("instance_name".into(), row.hostname.clone());
            labels.insert("provider".into(), row.provider_name);
            labels.insert("country".into(), row.country);

            if !row.alias.is_empty() {
                labels.insert("alias".into(), row.alias);
            }
            if !row.city.is_empty() {
                labels.insert("city".into(), row.city);
            }
            if !row.dc_name.is_empty() {
                labels.insert("dc".into(), row.dc_name);
            }
            if !row.purpose.is_empty() {
                labels.insert("purpose".into(), row.purpose);
            }
            if !row.vpn_protocol.is_empty() {
                labels.insert("vpn_protocol".into(), row.vpn_protocol);
            }
            labels.insert("status".into(), row.status);

            if !row.tags.is_empty() {
                labels.insert("tags".into(), row.tags.join(","));
            }
            if let Some(exp) = row.expire_date {
                labels.insert("expire_date".into(), exp.to_string());
            }

            Some(PrometheusTarget {
                targets: vec![target],
                labels,
            })
        })
        .collect();

    Ok(Json(targets))
}

#[derive(Debug, sqlx::FromRow)]
struct VpsTargetRow {
    hostname: String,
    alias: String,
    ip_addresses: sqlx::types::Json<Vec<IpEntry>>,
    node_exporter_port: i32,
    country: String,
    city: String,
    dc_name: String,
    status: String,
    purpose: String,
    vpn_protocol: String,
    tags: Vec<String>,
    expire_date: Option<chrono::NaiveDate>,
    provider_name: String,
}
