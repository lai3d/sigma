mod client;
mod config;
mod metrics;
mod models;
mod port_scan;
mod system;

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

    let client = SigmaClient::new(&config)?;

    // Initial registration
    match register(&client, &hostname, &config).await {
        Ok(vps) => info!(id = %vps.id, hostname = %vps.hostname, "Registered with sigma"),
        Err(e) => error!("Initial registration failed: {:#}", e),
    }

    // Heartbeat loop
    loop {
        tokio::time::sleep(Duration::from_secs(config.interval)).await;

        match heartbeat(&client, &hostname).await {
            Ok(_) => info!(hostname = %hostname, "Heartbeat sent"),
            Err(e) => warn!("Heartbeat failed: {:#}", e),
        }
    }
}

async fn register(client: &SigmaClient, hostname: &str, config: &Config) -> Result<VpsResponse> {
    let system_info = system::collect_system_info();
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

async fn heartbeat(client: &SigmaClient, hostname: &str) -> Result<VpsResponse> {
    let system_info = system::collect_system_info();

    let body = AgentHeartbeat {
        hostname: hostname.to_string(),
        system_info,
    };

    client
        .post::<_, VpsResponse>("/agent/heartbeat", &body)
        .await
}
