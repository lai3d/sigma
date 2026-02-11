mod checker;
mod client;
mod config;
mod models;

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

use crate::checker::CheckResult;
use crate::client::SigmaClient;
use crate::config::Config;
use crate::models::{CreateIpCheck, IpCheck, PaginatedResponse, Vps};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = Config::parse();
    let check_types = config.check_types();

    info!(
        source = %config.source,
        interval = config.interval,
        check_types = ?check_types,
        concurrency = config.concurrency,
        "sigma-probe starting"
    );

    let client = SigmaClient::new(&config)?;

    loop {
        if let Err(e) = run_probe_cycle(&client, &config, &check_types).await {
            error!("Probe cycle failed: {:#}", e);
        }

        info!("Sleeping {} seconds until next cycle", config.interval);
        tokio::time::sleep(Duration::from_secs(config.interval)).await;
    }
}

async fn run_probe_cycle(
    client: &SigmaClient,
    config: &Config,
    check_types: &[String],
) -> Result<()> {
    // 1. Fetch all active VPS with pagination
    let mut all_vps: Vec<Vps> = Vec::new();
    let mut page = 1;
    let per_page = 100;

    loop {
        let path = format!("/vps?status=active&page={}&per_page={}", page, per_page);
        let resp: PaginatedResponse<Vps> = client.get(&path).await?;
        let count = resp.data.len();
        all_vps.extend(resp.data);
        if (page * per_page) >= resp.total {
            break;
        }
        page += 1;
        if count == 0 {
            break;
        }
    }

    info!("Fetched {} active VPS instances", all_vps.len());

    if all_vps.is_empty() {
        info!("No active VPS found, skipping checks");
        return Ok(());
    }

    // 2. Collect (vps_id, ip, ssh_port) tuples
    struct Target {
        vps_id: uuid::Uuid,
        ip: IpAddr,
        ip_str: String,
        ssh_port: u16,
    }

    let mut targets: Vec<Target> = Vec::new();
    for vps in &all_vps {
        for entry in &vps.ip_addresses {
            if let Ok(ip) = entry.ip.parse::<IpAddr>() {
                targets.push(Target {
                    vps_id: vps.id,
                    ip,
                    ip_str: entry.ip.clone(),
                    ssh_port: vps.ssh_port as u16,
                });
            } else {
                warn!("Invalid IP address '{}' on VPS {}", entry.ip, vps.id);
            }
        }
    }

    info!(
        "Running checks on {} IPs with types {:?}",
        targets.len(),
        check_types
    );

    // 3. Run checks with concurrency control
    let semaphore = Arc::new(Semaphore::new(config.concurrency));
    let tcp_timeout = Duration::from_secs(config.tcp_timeout);
    let http_timeout = Duration::from_secs(config.http_timeout);
    let icmp_timeout = Duration::from_secs(3);
    let source = config.source.clone();

    let mut handles = Vec::new();

    for target in &targets {
        for check_type in check_types {
            let sem = semaphore.clone();
            let ip = target.ip;
            let ip_str = target.ip_str.clone();
            let vps_id = target.vps_id;
            let ssh_port = target.ssh_port;
            let check_type = check_type.clone();
            let source = source.clone();

            let handle = tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();

                let result: CheckResult = match check_type.as_str() {
                    "icmp" => checker::check_icmp(ip, icmp_timeout).await,
                    "tcp" => checker::check_tcp(ip, ssh_port, tcp_timeout).await,
                    "http" => checker::check_http(ip, http_timeout).await,
                    _ => CheckResult {
                        success: false,
                        latency_ms: None,
                    },
                };

                CreateIpCheck {
                    vps_id,
                    ip: ip_str,
                    check_type: Some(check_type),
                    source: Some(source),
                    success: result.success,
                    latency_ms: result.latency_ms,
                }
            });

            handles.push(handle);
        }
    }

    // 4. Collect results and POST to API
    let mut total = 0u32;
    let mut success = 0u32;
    let mut failed = 0u32;
    let mut errors = 0u32;

    for handle in handles {
        let check = handle.await?;
        total += 1;
        if check.success {
            success += 1;
        } else {
            failed += 1;
        }

        match client.post::<_, IpCheck>("/ip-checks", &check).await {
            Ok(_) => {}
            Err(e) => {
                errors += 1;
                warn!(
                    "Failed to report check for {} ({}): {:#}",
                    check.ip,
                    check.check_type.as_deref().unwrap_or("unknown"),
                    e
                );
            }
        }
    }

    // 5. Log summary
    info!(
        total = total,
        success = success,
        failed = failed,
        api_errors = errors,
        "Probe cycle complete"
    );

    Ok(())
}
