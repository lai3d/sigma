use std::fmt::Write;
use std::sync::Arc;

use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use tokio::net::TcpListener;
use tracing::{error, info};

use crate::port_scan::{PortScanResult, SharedScanResult};

/// Known sources that are always emitted (even when count=0) for stable time series
const KNOWN_SOURCES: &[&str] = &[
    "envoy",
    "sshd",
    "nginx",
    "node_exporter",
    "other",
    "unknown",
];

struct MetricsState {
    scan_result: SharedScanResult,
    hostname: String,
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
        "# HELP sigma_ports_available Number of available (free) ports"
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

    // sigma_ports_used (per source)
    writeln!(
        out,
        "# HELP sigma_ports_used Number of used ports by source process"
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

async fn metrics_handler(State(state): State<Arc<MetricsState>>) -> impl IntoResponse {
    let result = state.scan_result.read().await;
    let body = render_metrics(&result, &state.hostname);
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}

/// Start the metrics HTTP server on the given port
pub async fn serve_metrics(port: u16, scan_result: SharedScanResult, hostname: String) {
    let state = Arc::new(MetricsState {
        scan_result,
        hostname,
    });

    let app = Router::new()
        .route("/metrics", get(metrics_handler))
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
            scan_duration: Duration::from_secs(1),
        };
        let output = render_metrics(&result, "h");

        // Extra source should appear
        assert!(output.contains("sigma_ports_used{hostname=\"h\",source=\"haproxy\"} 5"));
        // Known sources still appear
        assert!(output.contains("sigma_ports_used{hostname=\"h\",source=\"envoy\"} 0"));
    }
}
