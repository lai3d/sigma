use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::process::Command;
use tokio::sync::RwLock;
use tokio::task::spawn_blocking;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct PortScanResult {
    pub total_ports: u32,
    pub available: u32,
    pub used_by_source: HashMap<String, u32>,
    /// Breakdown of actual process names classified as "other" (for diagnostics)
    pub other_detail: HashMap<String, u32>,
    pub scan_duration: Duration,
}

impl Default for PortScanResult {
    fn default() -> Self {
        Self {
            total_ports: 0,
            available: 0,
            used_by_source: HashMap::new(),
            other_detail: HashMap::new(),
            scan_duration: Duration::ZERO,
        }
    }
}

pub type SharedScanResult = Arc<RwLock<PortScanResult>>;

/// Known process categories
const KNOWN_PROCESSES: &[&str] = &["envoy", "sshd", "nginx", "node_exporter"];

/// Classify a process name into a known category or "other"
fn classify_process(name: &str) -> &'static str {
    for &known in KNOWN_PROCESSES {
        if name.contains(known) {
            return known;
        }
    }
    "other"
}

/// Try to bind to a port; returns true if the port is available
pub fn try_bind(port: u16) -> bool {
    TcpListener::bind(("0.0.0.0", port)).is_ok()
}

/// Find N available ports in the given range by real-time bind testing
pub fn find_available_ports(start: u16, end: u16, count: usize) -> Vec<u16> {
    let mut ports = Vec::with_capacity(count);
    for port in start..=end {
        if try_bind(port) {
            ports.push(port);
            if ports.len() == count {
                break;
            }
        }
    }
    ports
}

/// Extract process name from ss users field like: users:(("envoy",pid=1234,fd=5))
fn extract_process_name(users_field: &str) -> Option<&str> {
    // Find ((" and extract the process name between ((" and "
    let start = users_field.find("((\"")? + 3;
    let rest = &users_field[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

/// Parse a single ss output line, returning (port, process_name) if successful.
/// Handles both `ss -tlnp` (no Netid column) and `ss -tulnp` (with Netid column):
///   LISTEN  0  4096  0.0.0.0:10001  0.0.0.0:*  users:(("envoy",...))
///   tcp  LISTEN  0  4096  0.0.0.0:10001  0.0.0.0:*  users:(("envoy",...))
fn parse_ss_line(line: &str) -> Option<(u16, String)> {
    let fields: Vec<&str> = line.split_whitespace().collect();

    // Find the position of "LISTEN" in the first two fields
    let listen_idx = if fields.first() == Some(&"LISTEN") {
        0
    } else if fields.get(1) == Some(&"LISTEN") {
        1
    } else {
        return None;
    };

    // Local address:port is 3 fields after LISTEN (Recv-Q, Send-Q, then addr)
    let addr_idx = listen_idx + 3;
    let local_addr = fields.get(addr_idx)?;
    let port_str = local_addr.rsplit(':').next()?;
    let port: u16 = port_str.parse().ok()?;

    // Process info is 2 fields after addr (peer addr, then users:...)
    let process_idx = addr_idx + 2;
    let process_name = if let Some(field) = fields.get(process_idx) {
        extract_process_name(field).unwrap_or("unknown").to_string()
    } else {
        "unknown".to_string()
    };

    Some((port, process_name))
}

/// Run `ss -tulnp` and parse output into a port->process_name map
async fn parse_ss_output() -> HashMap<u16, String> {
    let mut map = HashMap::new();

    let output = match Command::new("ss")
        .args(["-tulnp"])
        .output()
        .await
    {
        Ok(o) => o,
        Err(_) => return map,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines().skip(1) {
        if let Some((port, process)) = parse_ss_line(line) {
            map.insert(port, process);
        }
    }

    map
}

// ─── /proc-based port→process resolution ─────────────────────────────────

/// Parse /proc/net/tcp and /proc/net/tcp6 for listening sockets (state 0A).
/// Returns HashMap<inode, port>.
fn parse_proc_net_tcp(proc_path: &str) -> HashMap<u64, u16> {
    let mut inode_to_port = HashMap::new();

    let tcp_path = format!("{}/net/tcp", proc_path);
    let tcp6_path = format!("{}/net/tcp6", proc_path);
    for path in &[tcp_path.as_str(), tcp6_path.as_str()] {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for line in content.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 10 {
                continue;
            }

            // st field (index 3) must be "0A" (TCP LISTEN)
            if fields[3] != "0A" {
                continue;
            }

            // Parse port from local_address (hex: ADDR:PORT)
            let local_addr = fields[1];
            let port_hex = match local_addr.rsplit(':').next() {
                Some(h) => h,
                None => continue,
            };
            let port = match u16::from_str_radix(port_hex, 16) {
                Ok(p) => p,
                Err(_) => continue,
            };

            // Parse inode (index 9)
            if let Ok(inode) = fields[9].parse::<u64>() {
                if inode != 0 {
                    inode_to_port.insert(inode, port);
                }
            }
        }
    }

    inode_to_port
}

/// Walk /proc/<pid>/fd/ to map socket inodes → process names.
/// Returns HashMap<inode, process_name>.
fn scan_proc_fds(proc_path: &str, listening_inodes: &HashMap<u64, u16>) -> HashMap<u64, String> {
    let mut inode_to_process: HashMap<u64, String> = HashMap::new();

    let proc_dir = match std::fs::read_dir(proc_path) {
        Ok(d) => d,
        Err(e) => {
            warn!(error = %e, "Cannot read /proc");
            return inode_to_process;
        }
    };

    for entry in proc_dir.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Only numeric directories (PIDs)
        if !name_str.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let pid_path = entry.path();

        // Read process name from /proc/<pid>/comm
        let comm_path = pid_path.join("comm");
        let comm = match std::fs::read_to_string(&comm_path) {
            Ok(c) => c.trim().to_string(),
            Err(_) => continue,
        };

        // Read fd directory
        let fd_path = pid_path.join("fd");
        let fd_dir = match std::fs::read_dir(&fd_path) {
            Ok(d) => d,
            Err(_) => continue, // Permission denied — skip this PID
        };

        for fd_entry in fd_dir.flatten() {
            if let Ok(link) = std::fs::read_link(fd_entry.path()) {
                let link_str = link.to_string_lossy();
                // Format: socket:[12345]
                if let Some(inode_str) = link_str
                    .strip_prefix("socket:[")
                    .and_then(|s| s.strip_suffix(']'))
                {
                    if let Ok(inode) = inode_str.parse::<u64>() {
                        // Only record if this inode is a listening socket we care about
                        if listening_inodes.contains_key(&inode) {
                            inode_to_process
                                .entry(inode)
                                .or_insert_with(|| comm.clone());
                        }
                    }
                }
            }
        }
    }

    inode_to_process
}

/// Build port → process name mapping by reading /proc directly.
/// This bypasses `ss -p` and works even when ss can't resolve process names.
/// When `proc_path` points to a host-mounted /proc (e.g. /host/proc), this
/// works around Docker's procfs restrictions.
fn build_proc_port_map(proc_path: &str) -> HashMap<u16, String> {
    let inode_to_port = parse_proc_net_tcp(proc_path);
    if inode_to_port.is_empty() {
        return HashMap::new();
    }

    let inode_to_process = scan_proc_fds(proc_path, &inode_to_port);

    let mut port_to_process = HashMap::new();
    for (inode, port) in &inode_to_port {
        if let Some(process) = inode_to_process.get(inode) {
            port_to_process.insert(*port, process.clone());
        }
    }

    debug!(
        listening = inode_to_port.len(),
        resolved = port_to_process.len(),
        "proc port scan: resolved {}/{} listening ports",
        port_to_process.len(),
        inode_to_port.len()
    );

    port_to_process
}

// ─── Main scan logic ─────────────────────────────────────────────────────

/// Build a comprehensive port → process map using ss + /proc fallback.
async fn build_port_map(proc_path: &str) -> HashMap<u16, String> {
    let mut map = parse_ss_output().await;

    // Count how many ports ss couldn't resolve
    let unknown_count = map.values().filter(|v| v.as_str() == "unknown").count();

    if unknown_count > 0 {
        // Use /proc as fallback for ports where ss -p failed
        let pp = proc_path.to_string();
        let proc_map = spawn_blocking(move || build_proc_port_map(&pp))
            .await
            .unwrap_or_default();

        let mut resolved = 0u32;
        for (port, process) in &proc_map {
            match map.get(port) {
                Some(existing) if existing == "unknown" => {
                    // ss found the port but couldn't get process — use /proc result
                    map.insert(*port, process.clone());
                    resolved += 1;
                }
                None => {
                    // /proc found a port ss didn't see at all
                    map.insert(*port, process.clone());
                    resolved += 1;
                }
                _ => {} // ss already has a good process name — keep it
            }
        }

        if resolved > 0 {
            info!(
                resolved,
                remaining_unknown = unknown_count as u32 - resolved,
                "Resolved ports via /proc fallback"
            );
        }

        let still_unknown = map.values().filter(|v| v.as_str() == "unknown").count();
        if still_unknown > 0 {
            warn!(
                count = still_unknown,
                "Ports with unknown process (ss -p and /proc both failed — check permissions)"
            );
        }
    }

    map
}

/// Scan ports in the given range, returning aggregated results.
///
/// `used_by_source` counts ALL listening ports per process (system-wide),
/// while `available`/`total_ports` reflect the configured scan range only.
async fn scan_ports(start: u16, end: u16, proc_path: &str) -> PortScanResult {
    let start_time = Instant::now();
    let port_map = build_port_map(proc_path).await;

    let total_ports = (end as u32) - (start as u32) + 1;

    // Count ALL listening ports by process (system-wide)
    let mut used_by_source: HashMap<String, u32> = HashMap::new();
    let mut other_detail: HashMap<String, u32> = HashMap::new();
    for name in port_map.values() {
        // "unknown" means neither ss nor /proc could resolve the process
        let source = if name == "unknown" {
            "unknown"
        } else {
            classify_process(name)
        };
        *used_by_source.entry(source.to_string()).or_insert(0) += 1;
        if source == "other" {
            *other_detail.entry(name.clone()).or_insert(0) += 1;
        }
    }

    // Run bind tests in a blocking task to count available ports within scan range
    let available = spawn_blocking(move || {
        let mut available = 0u32;
        for port in start..=end {
            if try_bind(port) {
                available += 1;
            }
        }
        available
    })
    .await
    .unwrap_or(total_ports);

    PortScanResult {
        total_ports,
        available,
        used_by_source,
        other_detail,
        scan_duration: start_time.elapsed(),
    }
}

/// Background loop: scan ports every `interval` seconds and store results
pub async fn scan_loop(shared: SharedScanResult, start: u16, end: u16, interval: u64, proc_path: String) {
    loop {
        let result = scan_ports(start, end, &proc_path).await;

        let used: u32 = result.used_by_source.values().sum();
        info!(
            total = result.total_ports,
            available = result.available,
            used = used,
            duration_ms = result.scan_duration.as_millis() as u64,
            "Port scan complete"
        );

        // Log detailed breakdown of "other" processes for diagnostics
        if let Some(&other_count) = result.used_by_source.get("other") {
            if other_count > 0 {
                info!(
                    breakdown = ?result.other_detail,
                    "Port scan: 'other' process breakdown"
                );
            }
        }

        *shared.write().await = result;

        tokio::time::sleep(Duration::from_secs(interval)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_process_name() {
        assert_eq!(
            extract_process_name(r#"users:(("envoy",pid=1234,fd=5))"#),
            Some("envoy")
        );
        assert_eq!(
            extract_process_name(r#"users:(("sshd",pid=999,fd=3))"#),
            Some("sshd")
        );
        assert_eq!(extract_process_name(""), None);
    }

    #[test]
    fn test_parse_ss_line_ipv4() {
        // ss -tlnp format (no Netid column)
        let line = r#"LISTEN  0  4096  0.0.0.0:10001  0.0.0.0:*  users:(("envoy",pid=1234,fd=5))"#;
        let (port, process) = parse_ss_line(line).unwrap();
        assert_eq!(port, 10001);
        assert_eq!(process, "envoy");
    }

    #[test]
    fn test_parse_ss_line_with_netid() {
        // ss -tulnp format (with Netid column)
        let line = r#"tcp   LISTEN 0      4096         0.0.0.0:10008      0.0.0.0:*    users:(("envoy",pid=2209,fd=27))"#;
        let (port, process) = parse_ss_line(line).unwrap();
        assert_eq!(port, 10008);
        assert_eq!(process, "envoy");
    }

    #[test]
    fn test_parse_ss_line_with_netid_no_process() {
        let line = "tcp   LISTEN 0      4096         0.0.0.0:9912       0.0.0.0:*";
        let (port, process) = parse_ss_line(line).unwrap();
        assert_eq!(port, 9912);
        assert_eq!(process, "unknown");
    }

    #[test]
    fn test_parse_ss_line_ipv6() {
        let line = r#"LISTEN  0  128  [::]:22  [::]:*  users:(("sshd",pid=800,fd=4))"#;
        let (port, process) = parse_ss_line(line).unwrap();
        assert_eq!(port, 22);
        assert_eq!(process, "sshd");
    }

    #[test]
    fn test_parse_ss_line_wildcard() {
        let line = r#"LISTEN  0  4096  *:9100  *:*  users:(("node_exporter",pid=500,fd=7))"#;
        let (port, process) = parse_ss_line(line).unwrap();
        assert_eq!(port, 9100);
        assert_eq!(process, "node_exporter");
    }

    #[test]
    fn test_parse_ss_line_no_process() {
        let line = "LISTEN  0  4096  0.0.0.0:8080  0.0.0.0:*";
        let (port, process) = parse_ss_line(line).unwrap();
        assert_eq!(port, 8080);
        assert_eq!(process, "unknown");
    }

    #[test]
    fn test_parse_ss_line_header_skipped() {
        let line = "State  Recv-Q  Send-Q  Local Address:Port  Peer Address:Port  Process";
        assert!(parse_ss_line(line).is_none());
    }

    #[test]
    fn test_parse_ss_line_netid_header_skipped() {
        let line = "Netid State  Recv-Q Send-Q Local Address:Port  Peer Address:Port Process";
        assert!(parse_ss_line(line).is_none());
    }

    #[test]
    fn test_parse_ss_line_udp_skipped() {
        let line = "udp   UNCONN 0      0          127.0.0.1:323        0.0.0.0:*";
        assert!(parse_ss_line(line).is_none());
    }

    #[test]
    fn test_classify_process() {
        assert_eq!(classify_process("envoy"), "envoy");
        assert_eq!(classify_process("sshd"), "sshd");
        assert_eq!(classify_process("nginx"), "nginx");
        assert_eq!(classify_process("node_exporter"), "node_exporter");
        assert_eq!(classify_process("/usr/bin/envoy"), "envoy");
        assert_eq!(classify_process("python3"), "other");
        assert_eq!(classify_process("java"), "other");
    }

    #[test]
    fn test_parse_proc_net_tcp_line() {
        // Verify hex port parsing logic
        assert_eq!(u16::from_str_radix("0050", 16).unwrap(), 80);
        assert_eq!(u16::from_str_radix("5968", 16).unwrap(), 22888);
        assert_eq!(u16::from_str_radix("0016", 16).unwrap(), 22);
    }
}
