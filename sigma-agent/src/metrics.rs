use std::fmt::Write;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::task::spawn_blocking;
use tracing::{error, info};

use crate::port_scan::{self, PortScanResult, SharedScanResult};

#[cfg(feature = "ebpf-traffic")]
use crate::ebpf_traffic::SharedTrafficStats;

/// Known sources that are always emitted (even when count=0) for stable time series
const KNOWN_SOURCES: &[&str] = &[
    "envoy",
    "sshd",
    "nginx",
    "node_exporter",
    "xray",
    "other",
    "unknown",
];

struct MetricsState {
    scan_result: SharedScanResult,
    hostname: String,
    port_range: Option<(u16, u16)>,
    #[cfg(feature = "ebpf-traffic")]
    traffic_stats: Option<SharedTrafficStats>,
}

#[derive(Deserialize)]
struct AllocateRequest {
    count: usize,
}

#[derive(Serialize)]
struct AllocateResponse {
    ports: Vec<u16>,
}

/// Render Prometheus text format from scan results
pub fn render_metrics(result: &PortScanResult, hostname: &str) -> String {
    let mut out = String::with_capacity(1024);

    // sigma_ports_total
    writeln!(out, "# HELP sigma_ports_total Total number of ports in scan range").unwrap();
    writeln!(out, "# TYPE sigma_ports_total gauge").unwrap();
    writeln!(
        out,
        "sigma_ports_total{{hostname=\"{}\"}} {}",
        hostname, result.total_ports
    )
    .unwrap();

    writeln!(out).unwrap();

    // sigma_ports_available
    writeln!(
        out,
        "# HELP sigma_ports_available Number of available (free) ports in scan range"
    )
    .unwrap();
    writeln!(out, "# TYPE sigma_ports_available gauge").unwrap();
    writeln!(
        out,
        "sigma_ports_available{{hostname=\"{}\"}} {}",
        hostname, result.available
    )
    .unwrap();

    writeln!(out).unwrap();

    // sigma_ports_in_use (total - available, includes TIME_WAIT etc.)
    let in_use = result.total_ports.saturating_sub(result.available);
    writeln!(
        out,
        "# HELP sigma_ports_in_use Occupied ports in scan range (LISTEN + TIME_WAIT + other states)"
    )
    .unwrap();
    writeln!(out, "# TYPE sigma_ports_in_use gauge").unwrap();
    writeln!(
        out,
        "sigma_ports_in_use{{hostname=\"{}\"}} {}",
        hostname, in_use
    )
    .unwrap();

    writeln!(out).unwrap();

    // sigma_ports_used (per source — system-wide from ss output)
    writeln!(
        out,
        "# HELP sigma_ports_used Number of listening ports by source process (system-wide)"
    )
    .unwrap();
    writeln!(out, "# TYPE sigma_ports_used gauge").unwrap();

    // Always emit known sources
    for &source in KNOWN_SOURCES {
        let count = result.used_by_source.get(source).copied().unwrap_or(0);
        writeln!(
            out,
            "sigma_ports_used{{hostname=\"{}\",source=\"{}\"}} {}",
            hostname, source, count
        )
        .unwrap();
    }

    // Emit any extra sources not in KNOWN_SOURCES
    for (source, count) in &result.used_by_source {
        if !KNOWN_SOURCES.contains(&source.as_str()) {
            writeln!(
                out,
                "sigma_ports_used{{hostname=\"{}\",source=\"{}\"}} {}",
                hostname, source, count
            )
            .unwrap();
        }
    }

    writeln!(out).unwrap();

    // sigma_ports_other_detail (breakdown of "other" processes)
    if !result.other_detail.is_empty() {
        writeln!(
            out,
            "# HELP sigma_ports_other_detail Listening port count by actual process name (unclassified)"
        )
        .unwrap();
        writeln!(out, "# TYPE sigma_ports_other_detail gauge").unwrap();
        for (process, count) in &result.other_detail {
            writeln!(
                out,
                "sigma_ports_other_detail{{hostname=\"{}\",process=\"{}\"}} {}",
                hostname, process, count
            )
            .unwrap();
        }
        writeln!(out).unwrap();
    }

    // sigma_port_scan_duration_seconds
    writeln!(
        out,
        "# HELP sigma_port_scan_duration_seconds Time taken for the last port scan"
    )
    .unwrap();
    writeln!(out, "# TYPE sigma_port_scan_duration_seconds gauge").unwrap();
    writeln!(
        out,
        "sigma_port_scan_duration_seconds{{hostname=\"{}\"}} {:.3}",
        hostname,
        result.scan_duration.as_secs_f64()
    )
    .unwrap();

    out
}

/// Render eBPF traffic metrics in Prometheus text format.
#[cfg(feature = "ebpf-traffic")]
pub fn render_traffic_metrics(stats: &[crate::ebpf_traffic::ProcessTraffic], hostname: &str) -> String {
    let mut out = String::with_capacity(512);

    writeln!(out, "# HELP sigma_traffic_bytes_sent_total TCP bytes sent by process (eBPF)").unwrap();
    writeln!(out, "# TYPE sigma_traffic_bytes_sent_total gauge").unwrap();
    for entry in stats {
        let container = entry.container_id.as_deref().unwrap_or("");
        writeln!(
            out,
            "sigma_traffic_bytes_sent_total{{hostname=\"{}\",process=\"{}\",container=\"{}\"}} {}",
            hostname, entry.process_name, container, entry.bytes_sent
        ).unwrap();
    }

    writeln!(out).unwrap();

    writeln!(out, "# HELP sigma_traffic_bytes_recv_total TCP bytes received by process (eBPF)").unwrap();
    writeln!(out, "# TYPE sigma_traffic_bytes_recv_total gauge").unwrap();
    for entry in stats {
        let container = entry.container_id.as_deref().unwrap_or("");
        writeln!(
            out,
            "sigma_traffic_bytes_recv_total{{hostname=\"{}\",process=\"{}\",container=\"{}\"}} {}",
            hostname, entry.process_name, container, entry.bytes_recv
        ).unwrap();
    }

    writeln!(out).unwrap();

    writeln!(out, "# HELP sigma_traffic_udp_bytes_sent_total UDP bytes sent by process (eBPF)").unwrap();
    writeln!(out, "# TYPE sigma_traffic_udp_bytes_sent_total gauge").unwrap();
    for entry in stats {
        let container = entry.container_id.as_deref().unwrap_or("");
        writeln!(
            out,
            "sigma_traffic_udp_bytes_sent_total{{hostname=\"{}\",process=\"{}\",container=\"{}\"}} {}",
            hostname, entry.process_name, container, entry.udp_bytes_sent
        ).unwrap();
    }

    writeln!(out).unwrap();

    writeln!(out, "# HELP sigma_traffic_udp_bytes_recv_total UDP bytes received by process (eBPF)").unwrap();
    writeln!(out, "# TYPE sigma_traffic_udp_bytes_recv_total gauge").unwrap();
    for entry in stats {
        let container = entry.container_id.as_deref().unwrap_or("");
        writeln!(
            out,
            "sigma_traffic_udp_bytes_recv_total{{hostname=\"{}\",process=\"{}\",container=\"{}\"}} {}",
            hostname, entry.process_name, container, entry.udp_bytes_recv
        ).unwrap();
    }

    writeln!(out).unwrap();

    writeln!(out, "# HELP sigma_tcp_retransmits_total TCP retransmit events by process (eBPF)").unwrap();
    writeln!(out, "# TYPE sigma_tcp_retransmits_total gauge").unwrap();
    for entry in stats {
        let container = entry.container_id.as_deref().unwrap_or("");
        writeln!(
            out,
            "sigma_tcp_retransmits_total{{hostname=\"{}\",process=\"{}\",container=\"{}\"}} {}",
            hostname, entry.process_name, container, entry.retransmits
        ).unwrap();
    }

    writeln!(out).unwrap();

    writeln!(out, "# HELP sigma_tcp_connections_active Current active TCP connections by process (eBPF)").unwrap();
    writeln!(out, "# TYPE sigma_tcp_connections_active gauge").unwrap();
    for entry in stats {
        let container = entry.container_id.as_deref().unwrap_or("");
        writeln!(
            out,
            "sigma_tcp_connections_active{{hostname=\"{}\",process=\"{}\",container=\"{}\"}} {}",
            hostname, entry.process_name, container, entry.active_connections
        ).unwrap();
    }

    writeln!(out).unwrap();

    writeln!(out, "# HELP sigma_tcp_connections_total Total TCP connections opened by process (eBPF)").unwrap();
    writeln!(out, "# TYPE sigma_tcp_connections_total counter").unwrap();
    for entry in stats {
        let container = entry.container_id.as_deref().unwrap_or("");
        writeln!(
            out,
            "sigma_tcp_connections_total{{hostname=\"{}\",process=\"{}\",container=\"{}\"}} {}",
            hostname, entry.process_name, container, entry.total_connections
        ).unwrap();
    }

    // Packet drop metrics — only emit for processes with drops
    let has_drops: Vec<_> = stats.iter().filter(|e| !e.drops.is_empty()).collect();
    if !has_drops.is_empty() {
        writeln!(out).unwrap();

        writeln!(out, "# HELP sigma_packet_drops_total Packet drops by process and reason (eBPF tracepoint skb:kfree_skb)").unwrap();
        writeln!(out, "# TYPE sigma_packet_drops_total gauge").unwrap();
        for entry in &has_drops {
            let container = entry.container_id.as_deref().unwrap_or("");
            for (reason, count) in &entry.drops {
                writeln!(
                    out,
                    "sigma_packet_drops_total{{hostname=\"{}\",process=\"{}\",container=\"{}\",reason=\"{}\"}} {}",
                    hostname, entry.process_name, container, reason, count
                ).unwrap();
            }
        }
    }

    // Connection latency metrics — only emit for processes with conn latency data
    let has_conn_latency: Vec<_> = stats.iter().filter(|e| e.conn_latency_avg_us > 0).collect();
    if !has_conn_latency.is_empty() {
        writeln!(out).unwrap();

        writeln!(out, "# HELP sigma_tcp_connection_latency_avg_us Average TCP connection latency (SYN-to-established) in microseconds by process (eBPF)").unwrap();
        writeln!(out, "# TYPE sigma_tcp_connection_latency_avg_us gauge").unwrap();
        for entry in &has_conn_latency {
            let container = entry.container_id.as_deref().unwrap_or("");
            writeln!(
                out,
                "sigma_tcp_connection_latency_avg_us{{hostname=\"{}\",process=\"{}\",container=\"{}\"}} {}",
                hostname, entry.process_name, container, entry.conn_latency_avg_us
            ).unwrap();
        }

        writeln!(out).unwrap();

        writeln!(out, "# HELP sigma_tcp_connection_latency_min_us Minimum TCP connection latency (SYN-to-established) in microseconds by process (eBPF)").unwrap();
        writeln!(out, "# TYPE sigma_tcp_connection_latency_min_us gauge").unwrap();
        for entry in &has_conn_latency {
            let container = entry.container_id.as_deref().unwrap_or("");
            writeln!(
                out,
                "sigma_tcp_connection_latency_min_us{{hostname=\"{}\",process=\"{}\",container=\"{}\"}} {}",
                hostname, entry.process_name, container, entry.conn_latency_min_us
            ).unwrap();
        }

        writeln!(out).unwrap();

        writeln!(out, "# HELP sigma_tcp_connection_latency_max_us Maximum TCP connection latency (SYN-to-established) in microseconds by process (eBPF)").unwrap();
        writeln!(out, "# TYPE sigma_tcp_connection_latency_max_us gauge").unwrap();
        for entry in &has_conn_latency {
            let container = entry.container_id.as_deref().unwrap_or("");
            writeln!(
                out,
                "sigma_tcp_connection_latency_max_us{{hostname=\"{}\",process=\"{}\",container=\"{}\"}} {}",
                hostname, entry.process_name, container, entry.conn_latency_max_us
            ).unwrap();
        }
    }

    // DNS query metrics — only emit for processes with DNS activity
    let has_dns: Vec<_> = stats.iter().filter(|e| e.dns_queries > 0).collect();
    if !has_dns.is_empty() {
        writeln!(out).unwrap();

        writeln!(out, "# HELP sigma_dns_queries_total DNS queries (UDP to port 53) by process (eBPF)").unwrap();
        writeln!(out, "# TYPE sigma_dns_queries_total gauge").unwrap();
        for entry in &has_dns {
            let container = entry.container_id.as_deref().unwrap_or("");
            writeln!(
                out,
                "sigma_dns_queries_total{{hostname=\"{}\",process=\"{}\",container=\"{}\"}} {}",
                hostname, entry.process_name, container, entry.dns_queries
            ).unwrap();
        }

        writeln!(out).unwrap();

        writeln!(out, "# HELP sigma_dns_bytes_total DNS query bytes (UDP to port 53) by process (eBPF)").unwrap();
        writeln!(out, "# TYPE sigma_dns_bytes_total gauge").unwrap();
        for entry in &has_dns {
            let container = entry.container_id.as_deref().unwrap_or("");
            writeln!(
                out,
                "sigma_dns_bytes_total{{hostname=\"{}\",process=\"{}\",container=\"{}\"}} {}",
                hostname, entry.process_name, container, entry.dns_bytes
            ).unwrap();
        }
    }

    // Exec metrics — only emit for processes with exec events
    let has_exec: Vec<_> = stats.iter().filter(|e| e.exec_count > 0).collect();
    if !has_exec.is_empty() {
        writeln!(out).unwrap();

        writeln!(out, "# HELP sigma_exec_total Process exec events by process (eBPF tracepoint sched:sched_process_exec)").unwrap();
        writeln!(out, "# TYPE sigma_exec_total gauge").unwrap();
        for entry in &has_exec {
            let container = entry.container_id.as_deref().unwrap_or("");
            writeln!(
                out,
                "sigma_exec_total{{hostname=\"{}\",process=\"{}\",container=\"{}\"}} {}",
                hostname, entry.process_name, container, entry.exec_count
            ).unwrap();
        }
    }

    // OOM kill metrics — only emit for processes with OOM kills
    let has_oom: Vec<_> = stats.iter().filter(|e| e.oom_kills > 0).collect();
    if !has_oom.is_empty() {
        writeln!(out).unwrap();

        writeln!(out, "# HELP sigma_oom_kills_total OOM kills by triggering process (eBPF tracepoint oom:mark_victim)").unwrap();
        writeln!(out, "# TYPE sigma_oom_kills_total gauge").unwrap();
        for entry in &has_oom {
            let container = entry.container_id.as_deref().unwrap_or("");
            writeln!(
                out,
                "sigma_oom_kills_total{{hostname=\"{}\",process=\"{}\",container=\"{}\"}} {}",
                hostname, entry.process_name, container, entry.oom_kills
            ).unwrap();
        }
    }

    // RTT metrics — only emit for processes with RTT data
    let has_rtt: Vec<_> = stats.iter().filter(|e| e.rtt_avg_us > 0).collect();
    if !has_rtt.is_empty() {
        writeln!(out).unwrap();

        writeln!(out, "# HELP sigma_tcp_rtt_avg_us Average TCP round-trip time in microseconds by process (eBPF)").unwrap();
        writeln!(out, "# TYPE sigma_tcp_rtt_avg_us gauge").unwrap();
        for entry in &has_rtt {
            let container = entry.container_id.as_deref().unwrap_or("");
            writeln!(
                out,
                "sigma_tcp_rtt_avg_us{{hostname=\"{}\",process=\"{}\",container=\"{}\"}} {}",
                hostname, entry.process_name, container, entry.rtt_avg_us
            ).unwrap();
        }

        writeln!(out).unwrap();

        writeln!(out, "# HELP sigma_tcp_rtt_min_us Minimum TCP round-trip time in microseconds by process (eBPF)").unwrap();
        writeln!(out, "# TYPE sigma_tcp_rtt_min_us gauge").unwrap();
        for entry in &has_rtt {
            let container = entry.container_id.as_deref().unwrap_or("");
            writeln!(
                out,
                "sigma_tcp_rtt_min_us{{hostname=\"{}\",process=\"{}\",container=\"{}\"}} {}",
                hostname, entry.process_name, container, entry.rtt_min_us
            ).unwrap();
        }

        writeln!(out).unwrap();

        writeln!(out, "# HELP sigma_tcp_rtt_max_us Maximum TCP round-trip time in microseconds by process (eBPF)").unwrap();
        writeln!(out, "# TYPE sigma_tcp_rtt_max_us gauge").unwrap();
        for entry in &has_rtt {
            let container = entry.container_id.as_deref().unwrap_or("");
            writeln!(
                out,
                "sigma_tcp_rtt_max_us{{hostname=\"{}\",process=\"{}\",container=\"{}\"}} {}",
                hostname, entry.process_name, container, entry.rtt_max_us
            ).unwrap();
        }
    }

    out
}

async fn metrics_handler(State(state): State<Arc<MetricsState>>) -> impl IntoResponse {
    let result = state.scan_result.read().await;

    #[allow(unused_mut)]
    let mut body = render_metrics(&result, &state.hostname);

    #[cfg(feature = "ebpf-traffic")]
    if let Some(ref traffic_stats) = state.traffic_stats {
        let stats = traffic_stats.read().await;
        if !stats.is_empty() {
            body.push('\n');
            body.push_str(&render_traffic_metrics(&stats, &state.hostname));
        }
    }

    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}

async fn allocate_handler(
    State(state): State<Arc<MetricsState>>,
    Json(req): Json<AllocateRequest>,
) -> Result<Json<AllocateResponse>, (StatusCode, String)> {
    let (start, end) = state.port_range.ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "Port scanning is not enabled".to_string(),
        )
    })?;

    if req.count == 0 || req.count > 1000 {
        return Err((
            StatusCode::BAD_REQUEST,
            "count must be between 1 and 1000".to_string(),
        ));
    }

    let count = req.count;
    let ports = spawn_blocking(move || port_scan::find_available_ports(start, end, count))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Scan failed: {}", e),
            )
        })?;

    if ports.len() < count {
        return Err((
            StatusCode::CONFLICT,
            format!(
                "Only {} available ports found, requested {}",
                ports.len(),
                count
            ),
        ));
    }

    Ok(Json(AllocateResponse { ports }))
}

/// Start the metrics HTTP server on the given port
pub async fn serve_metrics(
    port: u16,
    scan_result: SharedScanResult,
    hostname: String,
    port_range: Option<(u16, u16)>,
    #[cfg(feature = "ebpf-traffic")] traffic_stats: Option<SharedTrafficStats>,
    #[cfg(not(feature = "ebpf-traffic"))] _traffic_stats: Option<()>,
) {
    let state = Arc::new(MetricsState {
        scan_result,
        hostname,
        port_range,
        #[cfg(feature = "ebpf-traffic")]
        traffic_stats,
    });

    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/ports/allocate", post(allocate_handler))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            error!(port, error = %e, "Failed to bind metrics server");
            return;
        }
    };

    info!(port, "Metrics server listening");

    if let Err(e) = axum::serve(listener, app).await {
        error!(error = %e, "Metrics server error");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::Duration;

    #[test]
    fn test_render_metrics_empty() {
        let result = PortScanResult::default();
        let output = render_metrics(&result, "test-host");

        assert!(output.contains("sigma_ports_total{hostname=\"test-host\"} 0"));
        assert!(output.contains("sigma_ports_available{hostname=\"test-host\"} 0"));
        // All known sources should appear with 0
        assert!(output.contains("sigma_ports_used{hostname=\"test-host\",source=\"envoy\"} 0"));
        assert!(output.contains("sigma_ports_used{hostname=\"test-host\",source=\"sshd\"} 0"));
        assert!(output.contains("sigma_ports_used{hostname=\"test-host\",source=\"unknown\"} 0"));
        assert!(output.contains("sigma_port_scan_duration_seconds{hostname=\"test-host\"} 0.000"));
    }

    #[test]
    fn test_render_metrics_with_data() {
        let mut used = HashMap::new();
        used.insert("envoy".to_string(), 70);
        used.insert("sshd".to_string(), 1);
        used.insert("unknown".to_string(), 3);

        let result = PortScanResult {
            total_ports: 20001,
            available: 19927,
            used_by_source: used,
            other_detail: HashMap::new(),
            scan_duration: Duration::from_millis(287),
        };
        let output = render_metrics(&result, "relay-01");

        assert!(output.contains("sigma_ports_total{hostname=\"relay-01\"} 20001"));
        assert!(output.contains("sigma_ports_available{hostname=\"relay-01\"} 19927"));
        assert!(output.contains("sigma_ports_used{hostname=\"relay-01\",source=\"envoy\"} 70"));
        assert!(output.contains("sigma_ports_used{hostname=\"relay-01\",source=\"sshd\"} 1"));
        assert!(output.contains("sigma_ports_used{hostname=\"relay-01\",source=\"nginx\"} 0"));
        assert!(output.contains("sigma_ports_used{hostname=\"relay-01\",source=\"unknown\"} 3"));
        assert!(output.contains("sigma_port_scan_duration_seconds{hostname=\"relay-01\"} 0.287"));
    }

    #[test]
    fn test_render_metrics_has_help_and_type() {
        let result = PortScanResult::default();
        let output = render_metrics(&result, "h");

        assert!(output.contains("# HELP sigma_ports_total"));
        assert!(output.contains("# TYPE sigma_ports_total gauge"));
        assert!(output.contains("# HELP sigma_ports_available"));
        assert!(output.contains("# TYPE sigma_ports_available gauge"));
        assert!(output.contains("# HELP sigma_ports_used"));
        assert!(output.contains("# TYPE sigma_ports_used gauge"));
        assert!(output.contains("# HELP sigma_port_scan_duration_seconds"));
        assert!(output.contains("# TYPE sigma_port_scan_duration_seconds gauge"));
    }

    #[test]
    fn test_render_metrics_extra_source() {
        let mut used = HashMap::new();
        used.insert("haproxy".to_string(), 5);

        let result = PortScanResult {
            total_ports: 100,
            available: 95,
            used_by_source: used,
            other_detail: HashMap::new(),
            scan_duration: Duration::from_secs(1),
        };
        let output = render_metrics(&result, "h");

        // Extra source should appear
        assert!(output.contains("sigma_ports_used{hostname=\"h\",source=\"haproxy\"} 5"));
        // Known sources still appear
        assert!(output.contains("sigma_ports_used{hostname=\"h\",source=\"envoy\"} 0"));
    }
}
