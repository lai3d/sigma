use anyhow::Result;
use chrono::NaiveDate;
use uuid::Uuid;

use crate::client::SigmaClient;
use crate::models::*;
use crate::output;

pub async fn list(
    client: &SigmaClient,
    status: Option<&str>,
    country: Option<&str>,
    provider_id: Option<Uuid>,
    purpose: Option<&str>,
    tag: Option<&str>,
    expiring: Option<i32>,
    page: i64,
    per_page: i64,
    json: bool,
) -> Result<()> {
    let mut query = format!("/vps?page={page}&per_page={per_page}");
    if let Some(v) = status {
        query.push_str(&format!("&status={v}"));
    }
    if let Some(v) = country {
        query.push_str(&format!("&country={v}"));
    }
    if let Some(v) = provider_id {
        query.push_str(&format!("&provider_id={v}"));
    }
    if let Some(v) = purpose {
        query.push_str(&format!("&purpose={v}"));
    }
    if let Some(v) = tag {
        query.push_str(&format!("&tag={v}"));
    }
    if let Some(v) = expiring {
        query.push_str(&format!("&expiring_within_days={v}"));
    }

    let resp: PaginatedResponse<Vps> = client.get(&query).await?;

    if json {
        return output::print_json(&resp.data);
    }

    let rows: Vec<Vec<String>> = resp
        .data
        .iter()
        .map(|v| {
            let ips: String = v
                .ip_addresses
                .iter()
                .map(|e| e.ip.clone())
                .collect::<Vec<_>>()
                .join(", ");
            vec![
                v.id.to_string(),
                v.hostname.clone(),
                ips,
                v.country.clone(),
                v.status.clone(),
                v.purpose.clone(),
                v.expire_date
                    .map_or("-".into(), |d| d.to_string()),
            ]
        })
        .collect();

    output::print_table(
        &["ID", "Hostname", "IPs", "Country", "Status", "Purpose", "Expires"],
        rows,
    );
    output::print_pagination(resp.page, resp.per_page, resp.total);
    Ok(())
}

pub async fn get(client: &SigmaClient, id: Uuid, json: bool) -> Result<()> {
    let v: Vps = client.get(&format!("/vps/{id}")).await?;

    if json {
        return output::print_json(&v);
    }

    let ips: String = v
        .ip_addresses
        .iter()
        .map(|e| {
            if e.label.is_empty() {
                e.ip.clone()
            } else {
                format!("{} ({})", e.ip, e.label)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    output::print_table(
        &["Field", "Value"],
        vec![
            vec!["ID".into(), v.id.to_string()],
            vec!["Hostname".into(), v.hostname],
            vec!["Alias".into(), v.alias],
            vec!["Provider ID".into(), v.provider_id.to_string()],
            vec!["IPs".into(), ips],
            vec!["SSH Port".into(), v.ssh_port.to_string()],
            vec!["Country".into(), v.country],
            vec!["City".into(), v.city],
            vec!["DC".into(), v.dc_name],
            vec!["CPU Cores".into(), v.cpu_cores.map_or("-".into(), |n| n.to_string())],
            vec!["RAM (MB)".into(), v.ram_mb.map_or("-".into(), |n| n.to_string())],
            vec!["Disk (GB)".into(), v.disk_gb.map_or("-".into(), |n| n.to_string())],
            vec!["Bandwidth (TB)".into(), v.bandwidth_tb.map_or("-".into(), |n| n.to_string())],
            vec!["Cost/mo".into(), format!("{} {}", v.cost_monthly.map_or("-".into(), |n| n.to_string()), v.currency)],
            vec!["Status".into(), v.status],
            vec!["Purchase Date".into(), v.purchase_date.map_or("-".into(), |d| d.to_string())],
            vec!["Expire Date".into(), v.expire_date.map_or("-".into(), |d| d.to_string())],
            vec!["Purpose".into(), v.purpose],
            vec!["VPN Protocol".into(), v.vpn_protocol],
            vec!["Tags".into(), v.tags.join(", ")],
            vec!["Monitoring".into(), v.monitoring_enabled.to_string()],
            vec!["Node Exporter Port".into(), v.node_exporter_port.to_string()],
            vec!["Notes".into(), v.notes],
            vec!["Created".into(), v.created_at.to_rfc3339()],
            vec!["Updated".into(), v.updated_at.to_rfc3339()],
        ],
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn create(
    client: &SigmaClient,
    hostname: String,
    provider_id: Uuid,
    alias: Option<String>,
    ip: Vec<String>,
    ssh_port: Option<i32>,
    country: Option<String>,
    city: Option<String>,
    dc_name: Option<String>,
    cpu_cores: Option<i16>,
    ram_mb: Option<i32>,
    disk_gb: Option<i32>,
    bandwidth_tb: Option<f64>,
    cost_monthly: Option<f64>,
    currency: Option<String>,
    status: Option<String>,
    purchase_date: Option<NaiveDate>,
    expire_date: Option<NaiveDate>,
    purpose: Option<String>,
    vpn_protocol: Option<String>,
    tags: Vec<String>,
    monitoring_enabled: Option<bool>,
    node_exporter_port: Option<i32>,
    notes: Option<String>,
    json: bool,
) -> Result<()> {
    let ip_addresses = if ip.is_empty() {
        None
    } else {
        Some(
            ip.iter()
                .map(|s| {
                    // Support "ip:label" format
                    if let Some((addr, label)) = s.split_once(':') {
                        IpEntry { ip: addr.to_string(), label: label.to_string() }
                    } else {
                        IpEntry { ip: s.clone(), label: String::new() }
                    }
                })
                .collect(),
        )
    };

    let body = CreateVps {
        hostname,
        alias,
        provider_id,
        ip_addresses,
        ssh_port,
        country,
        city,
        dc_name,
        cpu_cores,
        ram_mb,
        disk_gb,
        bandwidth_tb,
        cost_monthly,
        currency,
        status,
        purchase_date,
        expire_date,
        purpose,
        vpn_protocol,
        tags: if tags.is_empty() { None } else { Some(tags) },
        monitoring_enabled,
        node_exporter_port,
        extra: None,
        notes,
    };

    let vps: Vps = client.post("/vps", &body).await?;

    if json {
        return output::print_json(&vps);
    }

    println!("Created VPS {} ({})", vps.hostname, vps.id);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn update(
    client: &SigmaClient,
    id: Uuid,
    hostname: Option<String>,
    alias: Option<String>,
    provider_id: Option<Uuid>,
    ip: Option<Vec<String>>,
    ssh_port: Option<i32>,
    country: Option<String>,
    city: Option<String>,
    dc_name: Option<String>,
    cpu_cores: Option<i16>,
    ram_mb: Option<i32>,
    disk_gb: Option<i32>,
    bandwidth_tb: Option<f64>,
    cost_monthly: Option<f64>,
    currency: Option<String>,
    status: Option<String>,
    purchase_date: Option<NaiveDate>,
    expire_date: Option<NaiveDate>,
    purpose: Option<String>,
    vpn_protocol: Option<String>,
    tags: Option<Vec<String>>,
    monitoring_enabled: Option<bool>,
    node_exporter_port: Option<i32>,
    notes: Option<String>,
    json: bool,
) -> Result<()> {
    let ip_addresses = ip.map(|ips| {
        ips.iter()
            .map(|s| {
                if let Some((addr, label)) = s.split_once(':') {
                    IpEntry { ip: addr.to_string(), label: label.to_string() }
                } else {
                    IpEntry { ip: s.clone(), label: String::new() }
                }
            })
            .collect()
    });

    let body = UpdateVps {
        hostname,
        alias,
        provider_id,
        ip_addresses,
        ssh_port,
        country,
        city,
        dc_name,
        cpu_cores: cpu_cores.map(Some),
        ram_mb: ram_mb.map(Some),
        disk_gb: disk_gb.map(Some),
        bandwidth_tb: bandwidth_tb.map(Some),
        cost_monthly: cost_monthly.map(Some),
        currency,
        status,
        purchase_date: purchase_date.map(Some),
        expire_date: expire_date.map(Some),
        purpose,
        vpn_protocol,
        tags,
        monitoring_enabled,
        node_exporter_port,
        extra: None,
        notes,
    };

    let vps: Vps = client.put(&format!("/vps/{id}"), &body).await?;

    if json {
        return output::print_json(&vps);
    }

    println!("Updated VPS {} ({})", vps.hostname, vps.id);
    Ok(())
}

pub async fn delete(client: &SigmaClient, id: Uuid) -> Result<()> {
    client.delete(&format!("/vps/{id}")).await?;
    println!("Deleted VPS {id}");
    Ok(())
}

pub async fn retire(client: &SigmaClient, id: Uuid, json: bool) -> Result<()> {
    let vps: Vps = client.post_empty(&format!("/vps/{id}/retire")).await?;

    if json {
        return output::print_json(&vps);
    }

    println!("Retired VPS {} ({})", vps.hostname, vps.id);
    Ok(())
}

pub async fn export(
    client: &SigmaClient,
    format: &str,
    output_file: Option<&str>,
) -> Result<()> {
    let (body, _) = client.get_text(&format!("/vps/export?format={format}")).await?;

    match output_file {
        Some(path) => {
            std::fs::write(path, &body)?;
            println!("Exported VPS to {path}");
        }
        None => print!("{body}"),
    }
    Ok(())
}

pub async fn import(client: &SigmaClient, file: &str, format: &str) -> Result<()> {
    let data = std::fs::read_to_string(file)?;
    let body = ImportRequest {
        format: format.to_string(),
        data,
    };

    let result: ImportResult = client.post("/vps/import", &body).await?;

    println!("Imported {} VPS instances", result.imported);
    if !result.errors.is_empty() {
        println!("Errors:");
        for err in &result.errors {
            println!("  - {err}");
        }
    }
    Ok(())
}
