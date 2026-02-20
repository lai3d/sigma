use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::process::Command;
use tokio::sync::RwLock;
use tokio::task::spawn_blocking;
use tracing::info;

#[derive(Debug, Clone)]
pub struct PortScanResult {
    pub total_ports: u32,
    pub available: u32,
    pub used_by_source: HashMap<String, u32>,
    pub scan_duration: Duration,
}

impl Default for PortScanResult {
    fn default() -> Self {
        Self {
            total_ports: 0,
            available: 0,
            used_by_source: HashMap::new(),
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

/// Scan ports in the given range, returning aggregated results
async fn scan_ports(start: u16, end: u16) -> PortScanResult {
    let start_time = Instant::now();
    let ss_map = parse_ss_output().await;

    let total_ports = (end as u32) - (start as u32) + 1;

    // Run bind tests in a blocking task to avoid blocking the async runtime
    let ss_map_clone = ss_map.clone();
    let (available, used_by_source) = spawn_blocking(move || {
        let mut available = 0u32;
        let mut used_by_source: HashMap<String, u32> = HashMap::new();

        for port in start..=end {
            if try_bind(port) {
                available += 1;
            } else {
                let source = match ss_map_clone.get(&port) {
                    Some(name) => classify_process(name),
                    None => "unknown",
                };
                *used_by_source.entry(source.to_string()).or_insert(0) += 1;
            }
        }

        (available, used_by_source)
    })
    .await
    .unwrap_or((total_ports, HashMap::new()));

    PortScanResult {
        total_ports,
        available,
        used_by_source,
        scan_duration: start_time.elapsed(),
    }
}

/// Background loop: scan ports every `interval` seconds and store results
pub async fn scan_loop(shared: SharedScanResult, start: u16, end: u16, interval: u64) {
    loop {
        let result = scan_ports(start, end).await;

        let used: u32 = result.used_by_source.values().sum();
        info!(
            total = result.total_ports,
            available = result.available,
            used = used,
            duration_ms = result.scan_duration.as_millis() as u64,
            "Port scan complete"
        );

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
}
