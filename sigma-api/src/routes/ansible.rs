use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use std::collections::HashMap;

use crate::errors::AppError;
use crate::models::IpEntry;
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/ansible/inventory", get(inventory))
}

/// Ansible dynamic inventory JSON output.
/// Groups hosts by: status, country, purpose, provider, and each tag.
/// Only includes VPS with status in (active, provisioning).
async fn inventory(State(state): State<AppState>) -> Result<Json<AnsibleInventory>, AppError> {
    let rows = sqlx::query_as::<_, VpsInventoryRow>(
        r#"SELECT
            v.hostname, v.alias, v.ip_addresses, v.ssh_port,
            v.country, v.city, v.dc_name, v.status, v.purpose, v.vpn_protocol,
            v.tags, v.cpu_cores, v.ram_mb, v.disk_gb,
            v.monitoring_enabled, v.node_exporter_port,
            COALESCE(p.name, '') as provider_name
           FROM vps v
           LEFT JOIN providers p ON p.id = v.provider_id
           WHERE v.status IN ('active', 'provisioning')
           ORDER BY v.hostname"#,
    )
    .fetch_all(&state.db)
    .await?;

    let mut all_hosts: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    let mut hostvars: HashMap<String, serde_json::Value> = HashMap::new();

    for row in &rows {
        let hostname = &row.hostname;
        all_hosts.push(hostname.clone());

        // First non-internal IP as ansible_host
        let ansible_host = row
            .ip_addresses
            .0
            .iter()
            .find(|e| e.label != "internal")
            .or(row.ip_addresses.0.first())
            .map(|e| e.ip.clone())
            .unwrap_or_default();

        // Group by status
        if !row.status.is_empty() {
            let group = format!("status_{}", slug(&row.status));
            groups.entry(group).or_default().push(hostname.clone());
        }

        // Group by country
        if !row.country.is_empty() {
            let group = format!("country_{}", slug(&row.country));
            groups.entry(group).or_default().push(hostname.clone());
        }

        // Group by purpose
        if !row.purpose.is_empty() {
            let group = format!("purpose_{}", slug(&row.purpose));
            groups.entry(group).or_default().push(hostname.clone());
        }

        // Group by provider
        if !row.provider_name.is_empty() {
            let group = format!("provider_{}", slug(&row.provider_name));
            groups.entry(group).or_default().push(hostname.clone());
        }

        // Group by each tag
        for tag in &row.tags {
            if !tag.is_empty() {
                let group = format!("tag_{}", slug(tag));
                groups.entry(group).or_default().push(hostname.clone());
            }
        }

        // Build hostvars
        let mut vars = serde_json::Map::new();
        vars.insert("ansible_host".into(), ansible_host.into());
        vars.insert("ansible_port".into(), row.ssh_port.into());

        if !row.alias.is_empty() {
            vars.insert("sigma_alias".into(), row.alias.clone().into());
        }
        vars.insert("sigma_country".into(), row.country.clone().into());
        if !row.city.is_empty() {
            vars.insert("sigma_city".into(), row.city.clone().into());
        }
        if !row.dc_name.is_empty() {
            vars.insert("sigma_dc".into(), row.dc_name.clone().into());
        }
        vars.insert("sigma_provider".into(), row.provider_name.clone().into());
        vars.insert("sigma_status".into(), row.status.clone().into());
        if !row.purpose.is_empty() {
            vars.insert("sigma_purpose".into(), row.purpose.clone().into());
        }
        if !row.vpn_protocol.is_empty() {
            vars.insert("sigma_vpn_protocol".into(), row.vpn_protocol.clone().into());
        }
        if !row.tags.is_empty() {
            vars.insert("sigma_tags".into(), row.tags.clone().into());
        }

        // All IP addresses with labels
        let ips: Vec<serde_json::Value> = row
            .ip_addresses
            .0
            .iter()
            .map(|e| serde_json::json!({"ip": e.ip, "label": e.label}))
            .collect();
        vars.insert("sigma_ip_addresses".into(), ips.into());

        if let Some(cores) = row.cpu_cores {
            vars.insert("sigma_cpu_cores".into(), cores.into());
        }
        if let Some(ram) = row.ram_mb {
            vars.insert("sigma_ram_mb".into(), ram.into());
        }
        if let Some(disk) = row.disk_gb {
            vars.insert("sigma_disk_gb".into(), disk.into());
        }
        vars.insert("sigma_monitoring_enabled".into(), row.monitoring_enabled.into());
        vars.insert("sigma_node_exporter_port".into(), row.node_exporter_port.into());

        hostvars.insert(hostname.clone(), serde_json::Value::Object(vars));
    }

    let mut inventory = AnsibleInventory {
        groups: HashMap::new(),
        meta: Meta { hostvars },
    };

    // "all" group
    inventory.groups.insert(
        "all".into(),
        AnsibleGroup { hosts: all_hosts },
    );

    for (name, hosts) in groups {
        inventory.groups.insert(name, AnsibleGroup { hosts });
    }

    Ok(Json(inventory))
}

/// Ansible dynamic inventory JSON structure.
/// Groups are flattened into the top level; `_meta.hostvars` holds per-host variables.
#[derive(Debug, Serialize)]
struct AnsibleInventory {
    #[serde(flatten)]
    groups: HashMap<String, AnsibleGroup>,
    #[serde(rename = "_meta")]
    meta: Meta,
}

#[derive(Debug, Serialize)]
struct AnsibleGroup {
    hosts: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Meta {
    hostvars: HashMap<String, serde_json::Value>,
}

/// Convert a string to a safe Ansible group name slug.
fn slug(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

#[derive(Debug, sqlx::FromRow)]
struct VpsInventoryRow {
    hostname: String,
    alias: String,
    ip_addresses: sqlx::types::Json<Vec<IpEntry>>,
    ssh_port: i32,
    country: String,
    city: String,
    dc_name: String,
    status: String,
    purpose: String,
    vpn_protocol: String,
    tags: Vec<String>,
    cpu_cores: Option<i16>,
    ram_mb: Option<i32>,
    disk_gb: Option<i32>,
    monitoring_enabled: bool,
    node_exporter_port: i32,
    provider_name: String,
}
