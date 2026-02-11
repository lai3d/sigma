use std::net::IpAddr;

use serde_json::json;

use crate::models::IpEntry;

/// Collect all system info as a JSON value.
pub fn collect_system_info() -> serde_json::Value {
    json!({
        "cpu_cores": cpu_cores().unwrap_or(0),
        "ram_mb": ram_mb().unwrap_or(0),
        "disk_gb": disk_gb().unwrap_or(0),
        "uptime_seconds": uptime_seconds().unwrap_or(0),
        "load_avg": load_avg().unwrap_or_default(),
    })
}

/// Get hostname from /proc or fallback.
pub fn get_hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| gethostname_fallback())
}

/// Discover non-loopback IP addresses from /proc/net/fib_trie.
pub fn discover_ips() -> Vec<IpEntry> {
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

    // Deduplicate
    ips.sort_by(|a, b| a.ip.cmp(&b.ip));
    ips.dedup_by(|a, b| a.ip == b.ip);
    ips
}

fn is_docker_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            // Docker default bridge: 172.17.0.0/16
            octets[0] == 172 && octets[1] == 17
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

fn disk_gb() -> Option<u64> {
    unsafe {
        let path = std::ffi::CString::new("/").ok()?;
        let mut stat: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(path.as_ptr(), &mut stat) == 0 {
            let total_bytes = stat.f_blocks as u64 * stat.f_frsize as u64;
            Some(total_bytes / (1024 * 1024 * 1024))
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
