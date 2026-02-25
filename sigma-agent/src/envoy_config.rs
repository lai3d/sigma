use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::warn;

use crate::models::StaticRouteEntry;

// ─── Envoy YAML structures (subset) ─────────────────

#[derive(Debug, Deserialize)]
struct EnvoyConfig {
    static_resources: Option<StaticResources>,
}

#[derive(Debug, Deserialize)]
struct StaticResources {
    #[serde(default)]
    listeners: Vec<Listener>,
    #[serde(default)]
    clusters: Vec<Cluster>,
}

#[derive(Debug, Deserialize)]
struct Listener {
    #[allow(dead_code)]
    name: Option<String>,
    address: Option<Address>,
    #[serde(default)]
    filter_chains: Vec<FilterChain>,
}

#[derive(Debug, Deserialize)]
struct Address {
    socket_address: Option<SocketAddress>,
}

#[derive(Debug, Deserialize)]
struct SocketAddress {
    port_value: Option<i32>,
    address: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FilterChain {
    #[serde(default)]
    filters: Vec<Filter>,
}

#[derive(Debug, Deserialize)]
struct Filter {
    name: Option<String>,
    typed_config: Option<TypedConfig>,
}

#[derive(Debug, Deserialize)]
struct TypedConfig {
    cluster: Option<String>,
    #[serde(rename = "@type")]
    _type_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Cluster {
    name: Option<String>,
    #[serde(rename = "type")]
    cluster_type: Option<String>,
    connect_timeout: Option<String>,
    load_assignment: Option<LoadAssignment>,
    transport_socket: Option<TransportSocket>,
}

#[derive(Debug, Deserialize)]
struct LoadAssignment {
    #[serde(default)]
    endpoints: Vec<LocalityEndpoint>,
}

#[derive(Debug, Deserialize)]
struct LocalityEndpoint {
    #[serde(default)]
    lb_endpoints: Vec<LbEndpoint>,
}

#[derive(Debug, Deserialize)]
struct LbEndpoint {
    endpoint: Option<Endpoint>,
}

#[derive(Debug, Deserialize)]
struct Endpoint {
    address: Option<Address>,
}

#[derive(Debug, Deserialize)]
struct TransportSocket {
    name: Option<String>,
    typed_config: Option<serde_yaml::Value>,
}

// ─── Cluster info extracted ──────────────────────────

struct ClusterInfo {
    backend_host: Option<String>,
    backend_port: Option<i32>,
    cluster_type: String,
    connect_timeout_secs: i32,
    proxy_protocol: i32,
}

// ─── Public API ──────────────────────────────────────

/// Parse an envoy.yaml file and extract static route entries.
pub fn parse_envoy_config(path: &Path) -> Result<Vec<StaticRouteEntry>> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let config: EnvoyConfig = serde_yaml::from_str(&contents)
        .with_context(|| format!("Failed to parse {}", path.display()))?;

    let static_resources = match config.static_resources {
        Some(sr) => sr,
        None => return Ok(Vec::new()),
    };

    // Build cluster lookup: name → ClusterInfo
    let mut cluster_map: HashMap<String, ClusterInfo> = HashMap::new();
    for cluster in &static_resources.clusters {
        let name = match &cluster.name {
            Some(n) => n.clone(),
            None => continue,
        };

        // Skip infrastructure clusters
        if name == "xds_cluster" {
            continue;
        }

        let (backend_host, backend_port) = extract_cluster_endpoint(cluster);
        let cluster_type = map_cluster_type(cluster.cluster_type.as_deref());
        let connect_timeout_secs = parse_duration_secs(cluster.connect_timeout.as_deref());
        let proxy_protocol = detect_proxy_protocol(&cluster.transport_socket);

        cluster_map.insert(
            name,
            ClusterInfo {
                backend_host,
                backend_port,
                cluster_type,
                connect_timeout_secs,
                proxy_protocol,
            },
        );
    }

    // Extract routes from listeners
    let mut routes = Vec::new();
    for listener in &static_resources.listeners {
        let listen_port = match listener
            .address
            .as_ref()
            .and_then(|a| a.socket_address.as_ref())
            .and_then(|s| s.port_value)
        {
            Some(p) => p,
            None => continue,
        };

        // Find the tcp_proxy filter to get the cluster name
        let cluster_name = find_tcp_proxy_cluster(listener);
        let cluster_name = match cluster_name {
            Some(c) => c,
            None => continue,
        };

        // Skip if this references the xds_cluster
        if cluster_name == "xds_cluster" {
            continue;
        }

        let name = format!("static-{}", listen_port);

        if let Some(info) = cluster_map.get(&cluster_name) {
            routes.push(StaticRouteEntry {
                name,
                listen_port,
                backend_host: info.backend_host.clone(),
                backend_port: info.backend_port,
                cluster_type: info.cluster_type.clone(),
                connect_timeout_secs: info.connect_timeout_secs,
                proxy_protocol: info.proxy_protocol,
            });
        } else {
            // Cluster not found in static_resources — emit route with defaults
            warn!(
                cluster = %cluster_name,
                listen_port,
                "Listener references unknown cluster, using defaults"
            );
            routes.push(StaticRouteEntry {
                name,
                listen_port,
                backend_host: None,
                backend_port: None,
                cluster_type: "logical_dns".to_string(),
                connect_timeout_secs: 5,
                proxy_protocol: 0,
            });
        }
    }

    Ok(routes)
}

// ─── Helpers ─────────────────────────────────────────

fn extract_cluster_endpoint(cluster: &Cluster) -> (Option<String>, Option<i32>) {
    if let Some(la) = &cluster.load_assignment {
        for ep in &la.endpoints {
            for lb in &ep.lb_endpoints {
                if let Some(endpoint) = &lb.endpoint {
                    if let Some(addr) = &endpoint.address {
                        if let Some(sa) = &addr.socket_address {
                            return (sa.address.clone(), sa.port_value);
                        }
                    }
                }
            }
        }
    }
    (None, None)
}

fn map_cluster_type(t: Option<&str>) -> String {
    match t {
        Some("STRICT_DNS") | Some("strict_dns") => "strict_dns".to_string(),
        Some("STATIC") | Some("static") => "static".to_string(),
        Some("LOGICAL_DNS") | Some("logical_dns") => "logical_dns".to_string(),
        Some("EDS") | Some("eds") => "eds".to_string(),
        Some("ORIGINAL_DST") | Some("original_dst") => "original_dst".to_string(),
        Some(other) => other.to_lowercase(),
        None => "logical_dns".to_string(),
    }
}

fn parse_duration_secs(d: Option<&str>) -> i32 {
    match d {
        Some(s) => {
            // Envoy duration: "5s", "0.5s", "30s", etc.
            let trimmed = s.trim_end_matches('s');
            trimmed.parse::<f64>().unwrap_or(5.0).round() as i32
        }
        None => 5,
    }
}

fn detect_proxy_protocol(ts: &Option<TransportSocket>) -> i32 {
    match ts {
        Some(ts) => {
            let name = ts.name.as_deref().unwrap_or("");
            if name.contains("proxy_protocol") || name.contains("upstream_proxy_protocol") {
                // Try to detect version from typed_config
                if let Some(cfg) = &ts.typed_config {
                    let yaml_str = serde_yaml::to_string(cfg).unwrap_or_default();
                    if yaml_str.contains("V2") || yaml_str.contains("v2") {
                        return 2;
                    }
                }
                1 // Default to v1
            } else {
                0
            }
        }
        None => 0,
    }
}

fn find_tcp_proxy_cluster(listener: &Listener) -> Option<String> {
    for fc in &listener.filter_chains {
        for filter in &fc.filters {
            let name = filter.name.as_deref().unwrap_or("");
            if name == "envoy.filters.network.tcp_proxy" {
                if let Some(tc) = &filter.typed_config {
                    return tc.cluster.clone();
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_simple_config() {
        let yaml = r#"
static_resources:
  listeners:
    - name: listener_30001
      address:
        socket_address:
          address: 0.0.0.0
          port_value: 30001
      filter_chains:
        - filters:
            - name: envoy.filters.network.tcp_proxy
              typed_config:
                "@type": type.googleapis.com/envoy.extensions.filters.network.tcp_proxy.v3.TcpProxy
                cluster: backend_30001
    - name: listener_30002
      address:
        socket_address:
          address: 0.0.0.0
          port_value: 30002
      filter_chains:
        - filters:
            - name: envoy.filters.network.tcp_proxy
              typed_config:
                "@type": type.googleapis.com/envoy.extensions.filters.network.tcp_proxy.v3.TcpProxy
                cluster: backend_30002
  clusters:
    - name: xds_cluster
      type: STATIC
      connect_timeout: 5s
      load_assignment:
        cluster_name: xds_cluster
        endpoints:
          - lb_endpoints:
              - endpoint:
                  address:
                    socket_address:
                      address: 127.0.0.1
                      port_value: 18000
    - name: backend_30001
      type: STRICT_DNS
      connect_timeout: 5s
      load_assignment:
        cluster_name: backend_30001
        endpoints:
          - lb_endpoints:
              - endpoint:
                  address:
                    socket_address:
                      address: hk01.example.com
                      port_value: 30002
    - name: backend_30002
      type: STATIC
      connect_timeout: 10s
      load_assignment:
        cluster_name: backend_30002
        endpoints:
          - lb_endpoints:
              - endpoint:
                  address:
                    socket_address:
                      address: 1.2.3.4
                      port_value: 443
"#;

        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(yaml.as_bytes()).unwrap();

        let routes = parse_envoy_config(tmp.path()).unwrap();
        assert_eq!(routes.len(), 2);

        let r1 = &routes[0];
        assert_eq!(r1.name, "static-30001");
        assert_eq!(r1.listen_port, 30001);
        assert_eq!(r1.backend_host.as_deref(), Some("hk01.example.com"));
        assert_eq!(r1.backend_port, Some(30002));
        assert_eq!(r1.cluster_type, "strict_dns");
        assert_eq!(r1.connect_timeout_secs, 5);

        let r2 = &routes[1];
        assert_eq!(r2.name, "static-30002");
        assert_eq!(r2.listen_port, 30002);
        assert_eq!(r2.backend_host.as_deref(), Some("1.2.3.4"));
        assert_eq!(r2.backend_port, Some(443));
        assert_eq!(r2.cluster_type, "static");
        assert_eq!(r2.connect_timeout_secs, 10);
    }

    #[test]
    fn test_skip_xds_cluster_listener() {
        let yaml = r#"
static_resources:
  listeners:
    - name: xds_listener
      address:
        socket_address:
          address: 0.0.0.0
          port_value: 9901
      filter_chains:
        - filters:
            - name: envoy.filters.network.tcp_proxy
              typed_config:
                cluster: xds_cluster
  clusters:
    - name: xds_cluster
      type: STATIC
      connect_timeout: 5s
"#;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(yaml.as_bytes()).unwrap();

        let routes = parse_envoy_config(tmp.path()).unwrap();
        assert_eq!(routes.len(), 0);
    }

    #[test]
    fn test_no_static_resources() {
        let yaml = r#"
node:
  id: test
  cluster: sigma
"#;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(yaml.as_bytes()).unwrap();

        let routes = parse_envoy_config(tmp.path()).unwrap();
        assert_eq!(routes.len(), 0);
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration_secs(Some("5s")), 5);
        assert_eq!(parse_duration_secs(Some("10s")), 10);
        assert_eq!(parse_duration_secs(Some("0.5s")), 1);
        assert_eq!(parse_duration_secs(None), 5);
    }
}
