use std::net::IpAddr;
use std::time::Duration;

use serde_json::json;
use tracing::{debug, warn};

use crate::models::IpEntry;

/// Collect all system info as a JSON value, with optional metrics port and public IP.
pub fn collect_system_info(metrics_port: u16, public_ip: Option<&str>) -> serde_json::Value {
    let (disk_total, disk_used) = disk_stats().unwrap_or((0, 0));
    let mut info = json!({
        "cpu_cores": cpu_cores().unwrap_or(0),
        "ram_mb": ram_mb().unwrap_or(0),
        "disk_gb": disk_total,
        "disk_used_gb": disk_used,
        "uptime_seconds": uptime_seconds().unwrap_or(0),
        "load_avg": load_avg().unwrap_or_default(),
    });
    if metrics_port > 0 {
        info["metrics_port"] = json!(metrics_port);
    }
    if let Some(ip) = public_ip {
        info["public_ip"] = json!(ip);
    }
    info
}

/// Get hostname: prefer /etc/host_hostname (host hostname mounted into container),
/// then /proc/sys/kernel/hostname, then libc fallback.
pub fn get_hostname() -> String {
    // In Docker, /proc/sys/kernel/hostname returns the container ID.
    // Users can mount the host's hostname via: -v /etc/hostname:/etc/host_hostname:ro
    if let Ok(s) = std::fs::read_to_string("/etc/host_hostname") {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| gethostname_fallback())
}

/// Discover non-loopback IP addresses from /proc/net/fib_trie,
/// then fetch public IP from external service if no public IP was found locally.
pub async fn discover_ips() -> Vec<IpEntry> {
    let mut ips = Vec::new();

    if let Ok(content) = std::fs::read_to_string("/proc/net/fib_trie") {
        let mut current_prefix = String::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("|--") || trimmed.starts_with("+--") {
                current_prefix = trimmed
                    .trim_start_matches("|--")
                    .trim_start_matches("+--")
                    .trim()
                    .split('/')
                    .next()
                    .unwrap_or("")
                    .to_string();
            } else if trimmed.contains("/32 host LOCAL") {
                if let Ok(ip) = current_prefix.parse::<IpAddr>() {
                    if !ip.is_loopback() && !is_docker_ip(&ip) {
                        ips.push(IpEntry {
                            ip: ip.to_string(),
                            label: classify_ip(&ip),
                        });
                    }
                }
            }
        }
    }

    // If no public IP found locally, try external lookup
    let has_public = ips.iter().any(|e| {
        e.ip.parse::<IpAddr>()
            .map(|ip| !ip_is_private(&ip))
            .unwrap_or(false)
    });

    if !has_public {
        match fetch_public_ip().await {
            Ok(entry) => {
                debug!(ip = %entry.ip, "Discovered public IP via external service");
                ips.push(entry);
            }
            Err(e) => {
                warn!("Failed to discover public IP: {:#}", e);
            }
        }
    }

    // Deduplicate
    ips.sort_by(|a, b| a.ip.cmp(&b.ip));
    ips.dedup_by(|a, b| a.ip == b.ip);
    ips
}

/// Fetch default public IP from external services (the IP the world sees).
pub async fn fetch_public_ip() -> anyhow::Result<IpEntry> {
    let services = [
        "https://icanhazip.com",
        "https://ifconfig.me/ip",
        "https://api.ipify.org",
    ];

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    for url in &services {
        match client.get(*url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let text = resp.text().await.unwrap_or_default();
                let ip_str = text.trim();
                if let Ok(ip) = ip_str.parse::<IpAddr>() {
                    return Ok(IpEntry {
                        ip: ip.to_string(),
                        label: String::new(),
                    });
                }
            }
            Ok(resp) => {
                debug!(url, status = %resp.status(), "Public IP service returned error");
            }
            Err(e) => {
                debug!(url, error = %e, "Public IP service request failed");
            }
        }
    }

    anyhow::bail!("All public IP services failed")
}

fn ip_is_private(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local(),
        IpAddr::V6(_) => false,
    }
}

fn is_docker_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            // Docker bridge networks: 172.16.0.0/12 (172.16-31.x.x)
            octets[0] == 172 && (16..=31).contains(&octets[1])
        }
        _ => false,
    }
}

fn classify_ip(ip: &IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) if v4.is_private() => "internal".to_string(),
        _ => String::new(),
    }
}

fn cpu_cores() -> Option<u32> {
    let content = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    let count = content
        .lines()
        .filter(|line| line.starts_with("processor"))
        .count();
    Some(count as u32)
}

fn ram_mb() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in content.lines() {
        if line.starts_with("MemTotal:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let kb: u64 = parts.get(1)?.parse().ok()?;
            return Some(kb / 1024);
        }
    }
    None
}

fn disk_stats() -> Option<(u64, u64)> {
    unsafe {
        let path = std::ffi::CString::new("/").ok()?;
        let mut stat: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(path.as_ptr(), &mut stat) == 0 {
            let block_size = stat.f_frsize as u64;
            let total = stat.f_blocks as u64 * block_size / (1024 * 1024 * 1024);
            let used = (stat.f_blocks - stat.f_bfree) as u64 * block_size / (1024 * 1024 * 1024);
            Some((total, used))
        } else {
            None
        }
    }
}

fn uptime_seconds() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/uptime").ok()?;
    let secs: f64 = content.split_whitespace().next()?.parse().ok()?;
    Some(secs as u64)
}

fn load_avg() -> Option<Vec<f64>> {
    let content = std::fs::read_to_string("/proc/loadavg").ok()?;
    let parts: Vec<f64> = content
        .split_whitespace()
        .take(3)
        .filter_map(|s| s.parse().ok())
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts)
    }
}

fn gethostname_fallback() -> String {
    // Use libc gethostname as fallback (works on macOS too)
    let mut buf = vec![0u8; 256];
    unsafe {
        if libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) == 0 {
            let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
            String::from_utf8_lossy(&buf[..len]).to_string()
        } else {
            "unknown".to_string()
        }
    }
}
