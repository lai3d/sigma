mod client;
mod config;
mod envoy_config;
mod metrics;
mod models;
mod port_scan;
mod system;
mod xds;
mod xds_resources;

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::client::SigmaClient;
use crate::config::Config;
use crate::models::{AgentHeartbeat, AgentRegister, VpsResponse};
use crate::port_scan::{PortScanResult, SharedScanResult};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = Config::parse();

    let hostname = config
        .hostname
        .clone()
        .unwrap_or_else(|| system::get_hostname());

    info!(
        hostname = %hostname,
        interval = config.interval,
        "sigma-agent starting"
    );

    // Shared port scan result
    let scan_result: SharedScanResult = Arc::new(RwLock::new(PortScanResult::default()));

    // Conditionally start port scanning
    let port_range = if config.port_scan {
        let (start, end) = config.parse_port_scan_range()?;
        info!(range = %config.port_scan_range, interval = config.port_scan_interval, "Port scanning enabled");
        let shared = scan_result.clone();
        let interval = config.port_scan_interval;
        tokio::spawn(async move {
            port_scan::scan_loop(shared, start, end, interval).await;
        });
        Some((start, end))
    } else {
        None
    };

    // Conditionally start metrics server
    if config.metrics_port > 0 {
        let shared = scan_result.clone();
        let port = config.metrics_port;
        let hn = hostname.clone();
        tokio::spawn(async move {
            metrics::serve_metrics(port, shared, hn, port_range).await;
        });
    }

    // Fetch public IP once at startup (the IP the world sees via default route)
    let public_ip = match system::fetch_public_ip().await {
        Ok(entry) => {
            info!(ip = %entry.ip, "Detected public IP");
            Some(entry.ip)
        }
        Err(e) => {
            warn!("Failed to detect public IP: {:#}", e);
            None
        }
    };

    let client = Arc::new(SigmaClient::new(&config)?);

    // Initial registration
    let vps_id = match register(&client, &hostname, &config, public_ip.as_deref()).await {
        Ok(vps) => {
            info!(id = %vps.id, hostname = %vps.hostname, "Registered with sigma");
            Some(vps.id)
        }
        Err(e) => {
            error!("Initial registration failed: {:#}", e);
            None
        }
    };

    // Conditionally start xDS gRPC server
    if config.xds_enabled {
        let vps_id = vps_id.expect(
            "VPS registration must succeed before starting xDS server",
        );

        info!(
            port = config.xds_port,
            poll_interval = config.xds_poll_interval,
            "xDS server enabled (serves all envoy nodes for this VPS)"
        );

        let xds_server =
            xds::XdsServer::new(client.clone(), vps_id, config.xds_poll_interval);

        tokio::spawn(xds_server.clone().config_poll_loop());
        tokio::spawn(async move {
            if let Err(e) = xds::serve_xds(config.xds_port, xds_server).await {
                error!("xDS gRPC server failed: {:#}", e);
            }
        });
    }

    // Conditionally start envoy static config sync
    if config.envoy_config_sync {
        let vps_id = vps_id.expect(
            "VPS registration must succeed before starting envoy config sync",
        );
        let paths: Vec<String> = config
            .envoy_config_path
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let sync_interval = config.envoy_config_sync_interval;

        info!(
            paths = ?paths,
            interval = sync_interval,
            "Envoy static config sync enabled"
        );

        // Spawn one sync loop per config file
        for path in paths {
            let sync_client = client.clone();
            let sync_hostname = hostname.clone();
            tokio::spawn(async move {
                envoy_config_sync_loop(
                    sync_client,
                    vps_id,
                    &sync_hostname,
                    &path,
                    sync_interval,
                )
                .await;
            });
        }
    }

    // Heartbeat loop
    loop {
        tokio::time::sleep(Duration::from_secs(config.interval)).await;

        match heartbeat(&client, &hostname, &config, public_ip.as_deref()).await {
            Ok(_) => info!(hostname = %hostname, "Heartbeat sent"),
            Err(e) => warn!("Heartbeat failed: {:#}", e),
        }
    }
}

async fn register(client: &SigmaClient, hostname: &str, config: &Config, public_ip: Option<&str>) -> Result<VpsResponse> {
    let system_info = system::collect_system_info(config.metrics_port, public_ip);
    let ip_addresses = system::discover_ips().await;

    info!(
        ips = ?ip_addresses.iter().map(|e| &e.ip).collect::<Vec<_>>(),
        "Discovered IPs"
    );

    let body = AgentRegister {
        hostname: hostname.to_string(),
        alias: config.alias.clone(),
        ip_addresses,
        ssh_port: config.ssh_port,
        system_info,
    };

    client
        .post::<_, VpsResponse>("/agent/register", &body)
        .await
}

async fn heartbeat(client: &SigmaClient, hostname: &str, config: &Config, public_ip: Option<&str>) -> Result<VpsResponse> {
    let system_info = system::collect_system_info(config.metrics_port, public_ip);

    let body = AgentHeartbeat {
        hostname: hostname.to_string(),
        system_info,
    };

    client
        .post::<_, VpsResponse>("/agent/heartbeat", &body)
        .await
}

async fn envoy_config_sync_loop(
    client: Arc<SigmaClient>,
    vps_id: uuid::Uuid,
    hostname: &str,
    config_path: &str,
    interval_secs: u64,
) {
    use crate::models::{SyncStaticRoutesRequest, SyncStaticRoutesResponse};
    use std::path::Path;
    use std::time::SystemTime;

    let path = Path::new(config_path);
    let mut last_mtime: Option<SystemTime> = None;

    // Derive node_id from filename stem to support multiple config files.
    // Single default path → "static-{hostname}" (backward compatible)
    // Multiple paths → "static-{hostname}-{stem}" (e.g. static-myhost-envoy-relay)
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("envoy");
    let node_id = if stem == "envoy" {
        format!("static-{}", hostname)
    } else {
        format!("static-{}-{}", hostname, stem)
    };

    // Ensure we have an envoy node for this VPS
    let envoy_node_id = match get_or_create_envoy_node(&client, vps_id, &node_id).await {
        Ok(id) => id,
        Err(e) => {
            error!(path = %config_path, "Failed to get/create envoy node for static sync: {:#}", e);
            return;
        }
    };

    info!(
        node_id = %node_id,
        envoy_node_id = %envoy_node_id,
        path = %config_path,
        "Envoy static config sync ready"
    );

    // Sync immediately on startup
    if path.exists() {
        match envoy_config::parse_envoy_config(path) {
            Ok(routes) => {
                info!(routes = routes.len(), path = %config_path, "Parsed envoy static config (initial)");
                let body = SyncStaticRoutesRequest {
                    envoy_node_id,
                    routes,
                };
                match client
                    .post::<_, SyncStaticRoutesResponse>("/envoy-routes/sync-static", &body)
                    .await
                {
                    Ok(resp) => info!(
                        upserted = resp.upserted,
                        deleted = resp.deleted,
                        path = %config_path,
                        "Static routes synced (initial)"
                    ),
                    Err(e) => warn!(path = %config_path, "Failed to sync static routes: {:#}", e),
                }
                last_mtime = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());
            }
            Err(e) => warn!(path = %config_path, "Failed to parse envoy config: {:#}", e),
        }
    } else {
        warn!(path = %config_path, "Envoy config file not found, will retry");
    }

    // Poll loop
    loop {
        tokio::time::sleep(Duration::from_secs(interval_secs)).await;

        if !path.exists() {
            continue;
        }

        // Check mtime
        let current_mtime = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());
        if current_mtime == last_mtime && last_mtime.is_some() {
            continue; // File unchanged
        }

        match envoy_config::parse_envoy_config(path) {
            Ok(routes) => {
                info!(routes = routes.len(), path = %config_path, "Envoy config changed, syncing");
                let body = SyncStaticRoutesRequest {
                    envoy_node_id,
                    routes,
                };
                match client
                    .post::<_, SyncStaticRoutesResponse>("/envoy-routes/sync-static", &body)
                    .await
                {
                    Ok(resp) => {
                        info!(
                            upserted = resp.upserted,
                            deleted = resp.deleted,
                            path = %config_path,
                            "Static routes synced"
                        );
                        last_mtime = current_mtime;
                    }
                    Err(e) => warn!(path = %config_path, "Failed to sync static routes: {:#}", e),
                }
            }
            Err(e) => warn!(path = %config_path, "Failed to parse envoy config: {:#}", e),
        }
    }
}

async fn get_or_create_envoy_node(
    client: &SigmaClient,
    vps_id: uuid::Uuid,
    node_id: &str,
) -> Result<uuid::Uuid> {
    use crate::models::{CreateEnvoyNode, EnvoyNode, PaginatedResponse};

    // Check if a node already exists for this VPS
    let nodes: PaginatedResponse<EnvoyNode> = client
        .get(&format!(
            "/envoy-nodes?vps_id={}&status=active&per_page=100",
            vps_id
        ))
        .await?;

    // Look for a node with matching node_id
    for node in &nodes.data {
        if node.node_id == node_id {
            return Ok(node.id);
        }
    }

    // Create a new node
    let body = CreateEnvoyNode {
        vps_id,
        node_id: node_id.to_string(),
        description: "Auto-registered for static config sync".to_string(),
    };

    let node: EnvoyNode = client.post("/envoy-nodes", &body).await?;
    Ok(node.id)
}
