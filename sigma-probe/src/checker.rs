use std::net::IpAddr;
use std::time::{Duration, Instant};

use surge_ping::{Client as PingClient, Config as PingConfig, PingIdentifier, PingSequence, ICMP};
use tokio::net::TcpStream;

pub struct CheckResult {
    pub success: bool,
    pub latency_ms: Option<i32>,
}

pub async fn check_icmp(ip: IpAddr, timeout: Duration) -> CheckResult {
    let config = PingConfig::builder()
        .kind(match ip {
            IpAddr::V4(_) => ICMP::V4,
            IpAddr::V6(_) => ICMP::V6,
        })
        .build();

    let client = match PingClient::new(&config) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to create ping client for {}: {}", ip, e);
            return CheckResult {
                success: false,
                latency_ms: None,
            };
        }
    };

    let payload = [0u8; 56];
    let mut pinger = client.pinger(ip, PingIdentifier(rand_id())).await;
    pinger.timeout(timeout);

    match pinger.ping(PingSequence(0), &payload).await {
        Ok((_, dur)) => CheckResult {
            success: true,
            latency_ms: Some(dur.as_millis() as i32),
        },
        Err(_) => CheckResult {
            success: false,
            latency_ms: None,
        },
    }
}

pub async fn check_tcp(ip: IpAddr, port: u16, timeout: Duration) -> CheckResult {
    let addr = std::net::SocketAddr::new(ip, port);
    let start = Instant::now();

    match tokio::time::timeout(timeout, TcpStream::connect(addr)).await {
        Ok(Ok(_)) => CheckResult {
            success: true,
            latency_ms: Some(start.elapsed().as_millis() as i32),
        },
        _ => CheckResult {
            success: false,
            latency_ms: None,
        },
    }
}

pub async fn check_http(ip: IpAddr, timeout: Duration) -> CheckResult {
    let client = match reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(timeout)
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            return CheckResult {
                success: false,
                latency_ms: None,
            }
        }
    };

    let url = format!("https://{}/", ip);
    let start = Instant::now();

    match client.get(&url).send().await {
        Ok(_) => CheckResult {
            success: true,
            latency_ms: Some(start.elapsed().as_millis() as i32),
        },
        Err(_) => CheckResult {
            success: false,
            latency_ms: None,
        },
    }
}

fn rand_id() -> u16 {
    use std::time::SystemTime;
    (SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos()
        & 0xFFFF) as u16
}
