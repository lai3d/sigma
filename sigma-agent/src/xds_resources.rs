use prost::Message;
use tracing::warn;
use xds_api::pb::envoy::config::{
    cluster::v3::{cluster, Cluster},
    core::v3::{address, socket_address, transport_socket, Address, SocketAddress, TransportSocket},
    endpoint::v3::{lb_endpoint, ClusterLoadAssignment, Endpoint, LbEndpoint, LocalityLbEndpoints},
    listener::v3::{filter, Filter, FilterChain, Listener},
};
use xds_api::pb::google::protobuf::{Any as XdsAny, Duration as XdsDuration};

use crate::models::EnvoyRoute;

// Type URLs for xDS resources
pub const CLUSTER_TYPE_URL: &str = "type.googleapis.com/envoy.config.cluster.v3.Cluster";
pub const LISTENER_TYPE_URL: &str = "type.googleapis.com/envoy.config.listener.v3.Listener";

const TCP_PROXY_TYPE_URL: &str =
    "type.googleapis.com/envoy.extensions.filters.network.tcp_proxy.v3.TcpProxy";
const PROXY_PROTOCOL_TRANSPORT_TYPE_URL: &str =
    "type.googleapis.com/envoy.extensions.transport_sockets.proxy_protocol.v3.ProxyProtocolUpstreamTransport";
const RAW_BUFFER_TYPE_URL: &str =
    "type.googleapis.com/envoy.extensions.transport_sockets.raw_buffer.v3.RawBuffer";

// Minimal proto message definitions not included in xds-api crate.
// We define them as prost::Message so they can be serialized to protobuf bytes.

/// envoy.extensions.filters.network.tcp_proxy.v3.TcpProxy (simplified)
#[derive(Clone, prost::Message)]
struct TcpProxy {
    #[prost(string, tag = "1")]
    pub stat_prefix: String,
    /// cluster_specifier oneof — we only use the `cluster` variant (tag 2)
    #[prost(string, tag = "2")]
    pub cluster: String,
}

/// envoy.extensions.transport_sockets.raw_buffer.v3.RawBuffer
#[derive(Clone, prost::Message)]
struct RawBuffer {}

/// envoy.extensions.transport_sockets.proxy_protocol.v3.ProxyProtocolConfig
#[derive(Clone, prost::Message)]
struct ProxyProtocolConfig {
    #[prost(enumeration = "ProxyProtocolVersion", tag = "1")]
    pub version: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
enum ProxyProtocolVersion {
    V1 = 0,
    V2 = 1,
}

/// envoy.extensions.transport_sockets.proxy_protocol.v3.ProxyProtocolUpstreamTransport
#[derive(Clone, prost::Message)]
struct ProxyProtocolUpstreamTransport {
    #[prost(message, optional, tag = "1")]
    pub config: Option<ProxyProtocolConfig>,
    #[prost(message, optional, tag = "2")]
    pub transport_socket: Option<TransportSocket>,
}

/// Encode a prost Message into an xDS Any.
fn encode_any<M: Message>(msg: &M, type_url: &str) -> XdsAny {
    XdsAny {
        type_url: type_url.to_string(),
        value: msg.encode_to_vec(),
    }
}

/// Map cluster_type string from DB to Envoy DiscoveryType enum value.
fn map_cluster_type(ct: &str) -> i32 {
    match ct {
        "static" => cluster::DiscoveryType::Static as i32,
        "strict_dns" => cluster::DiscoveryType::StrictDns as i32,
        "logical_dns" => cluster::DiscoveryType::LogicalDns as i32,
        _ => cluster::DiscoveryType::LogicalDns as i32,
    }
}

/// Build a socket address.
fn make_address(host: &str, port: u32) -> Option<Address> {
    Some(Address {
        address: Some(address::Address::SocketAddress(SocketAddress {
            address: host.to_string(),
            port_specifier: Some(socket_address::PortSpecifier::PortValue(port)),
            ..Default::default()
        })),
    })
}

/// Build a proxy protocol upstream transport socket.
/// `version`: DB value where 1=PPv1, 2=PPv2
fn build_proxy_protocol_transport(version: i32) -> TransportSocket {
    let inner = TransportSocket {
        name: "envoy.transport_sockets.raw_buffer".to_string(),
        config_type: Some(transport_socket::ConfigType::TypedConfig(encode_any(
            &RawBuffer {},
            RAW_BUFFER_TYPE_URL,
        ))),
    };

    let pp = ProxyProtocolUpstreamTransport {
        config: Some(ProxyProtocolConfig {
            // DB: 1=v1, 2=v2 → proto enum: V1=0, V2=1
            version: (version - 1).max(0),
        }),
        transport_socket: Some(inner),
    };

    TransportSocket {
        name: "envoy.transport_sockets.upstream_proxy_protocol".to_string(),
        config_type: Some(transport_socket::ConfigType::TypedConfig(encode_any(
            &pp,
            PROXY_PROTOCOL_TRANSPORT_TYPE_URL,
        ))),
    }
}

/// Build an xDS Cluster from an EnvoyRoute, encoded as XdsAny.
/// Returns None if the route has missing backend_host or backend_port.
pub fn build_cluster(route: &EnvoyRoute) -> Option<XdsAny> {
    let backend_host = match route.backend_host.as_deref() {
        Some(h) if !h.is_empty() => h,
        _ => {
            warn!(route = %route.name, "Skipping cluster: missing backend_host");
            return None;
        }
    };
    let backend_port = match route.backend_port {
        Some(p) if p > 0 => p as u32,
        _ => {
            warn!(route = %route.name, "Skipping cluster: missing backend_port");
            return None;
        }
    };

    let cluster_name = format!("cluster-{}", route.name);

    let cluster = Cluster {
        name: cluster_name.clone(),
        cluster_discovery_type: Some(cluster::ClusterDiscoveryType::Type(map_cluster_type(
            &route.cluster_type,
        ))),
        connect_timeout: Some(XdsDuration {
            seconds: route.connect_timeout_secs as i64,
            nanos: 0,
        }),
        load_assignment: Some(ClusterLoadAssignment {
            cluster_name: cluster_name.clone(),
            endpoints: vec![LocalityLbEndpoints {
                lb_endpoints: vec![LbEndpoint {
                    host_identifier: Some(lb_endpoint::HostIdentifier::Endpoint(Endpoint {
                        address: make_address(backend_host, backend_port),
                        ..Default::default()
                    })),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        transport_socket: if route.proxy_protocol > 0 {
            Some(build_proxy_protocol_transport(route.proxy_protocol))
        } else {
            None
        },
        ..Default::default()
    };

    Some(encode_any(&cluster, CLUSTER_TYPE_URL))
}

/// Build an xDS Listener from an EnvoyRoute, encoded as XdsAny.
pub fn build_listener(route: &EnvoyRoute) -> XdsAny {
    let cluster_name = format!("cluster-{}", route.name);
    let listener_name = format!("listener-{}", route.name);

    let tcp_proxy = TcpProxy {
        stat_prefix: format!("tcp-{}", route.name),
        cluster: cluster_name,
    };

    let listener = Listener {
        name: listener_name,
        address: make_address("0.0.0.0", route.listen_port as u32),
        filter_chains: vec![FilterChain {
            filters: vec![Filter {
                name: "envoy.filters.network.tcp_proxy".to_string(),
                config_type: Some(filter::ConfigType::TypedConfig(encode_any(
                    &tcp_proxy,
                    TCP_PROXY_TYPE_URL,
                ))),
            }],
            ..Default::default()
        }],
        ..Default::default()
    };

    encode_any(&listener, LISTENER_TYPE_URL)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_route() -> EnvoyRoute {
        EnvoyRoute {
            id: uuid::Uuid::new_v4(),
            envoy_node_id: uuid::Uuid::new_v4(),
            name: "test-route".to_string(),
            listen_port: 10001,
            backend_host: Some("backend.example.com".to_string()),
            backend_port: Some(8080),
            cluster_type: "logical_dns".to_string(),
            connect_timeout_secs: 5,
            proxy_protocol: 0,
            source: "dynamic".to_string(),
            status: "active".to_string(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_build_cluster_basic() {
        let route = test_route();
        let any = build_cluster(&route).expect("should build cluster");
        assert_eq!(any.type_url, CLUSTER_TYPE_URL);

        let cluster = Cluster::decode(any.value.as_slice()).expect("should decode");
        assert_eq!(cluster.name, "cluster-test-route");
        assert_eq!(
            cluster.cluster_discovery_type,
            Some(cluster::ClusterDiscoveryType::Type(
                cluster::DiscoveryType::LogicalDns as i32
            ))
        );
        assert_eq!(
            cluster.connect_timeout,
            Some(XdsDuration {
                seconds: 5,
                nanos: 0
            })
        );
        assert!(cluster.transport_socket.is_none());

        let la = cluster.load_assignment.expect("should have load_assignment");
        assert_eq!(la.cluster_name, "cluster-test-route");
        assert_eq!(la.endpoints.len(), 1);
        let ep = &la.endpoints[0].lb_endpoints[0];
        if let Some(lb_endpoint::HostIdentifier::Endpoint(ref endpoint)) = ep.host_identifier {
            let addr = endpoint.address.as_ref().unwrap();
            if let Some(address::Address::SocketAddress(ref sa)) = addr.address {
                assert_eq!(sa.address, "backend.example.com");
                assert_eq!(
                    sa.port_specifier,
                    Some(socket_address::PortSpecifier::PortValue(8080))
                );
            } else {
                panic!("expected SocketAddress");
            }
        } else {
            panic!("expected Endpoint host_identifier");
        }
    }

    #[test]
    fn test_build_cluster_with_proxy_protocol() {
        let mut route = test_route();
        route.proxy_protocol = 1; // PPv1

        let any = build_cluster(&route).expect("should build cluster");
        let cluster = Cluster::decode(any.value.as_slice()).expect("should decode");

        let ts = cluster.transport_socket.expect("should have transport_socket");
        assert_eq!(
            ts.name,
            "envoy.transport_sockets.upstream_proxy_protocol"
        );
        assert!(ts.config_type.is_some());
    }

    #[test]
    fn test_build_cluster_missing_backend() {
        let mut route = test_route();
        route.backend_host = None;
        assert!(build_cluster(&route).is_none());

        let mut route2 = test_route();
        route2.backend_port = None;
        assert!(build_cluster(&route2).is_none());
    }

    #[test]
    fn test_build_listener() {
        let route = test_route();
        let any = build_listener(&route);
        assert_eq!(any.type_url, LISTENER_TYPE_URL);

        let listener = Listener::decode(any.value.as_slice()).expect("should decode");
        assert_eq!(listener.name, "listener-test-route");

        let addr = listener.address.as_ref().unwrap();
        if let Some(address::Address::SocketAddress(ref sa)) = addr.address {
            assert_eq!(sa.address, "0.0.0.0");
            assert_eq!(
                sa.port_specifier,
                Some(socket_address::PortSpecifier::PortValue(10001))
            );
        } else {
            panic!("expected SocketAddress");
        }

        assert_eq!(listener.filter_chains.len(), 1);
        let fc = &listener.filter_chains[0];
        assert_eq!(fc.filters.len(), 1);
        assert_eq!(fc.filters[0].name, "envoy.filters.network.tcp_proxy");

        if let Some(filter::ConfigType::TypedConfig(ref any)) = fc.filters[0].config_type {
            assert_eq!(any.type_url, TCP_PROXY_TYPE_URL);
            let tcp = TcpProxy::decode(any.value.as_slice()).expect("should decode");
            assert_eq!(tcp.stat_prefix, "tcp-test-route");
            assert_eq!(tcp.cluster, "cluster-test-route");
        } else {
            panic!("expected TypedConfig");
        }
    }

    #[test]
    fn test_map_cluster_type() {
        assert_eq!(
            map_cluster_type("static"),
            cluster::DiscoveryType::Static as i32
        );
        assert_eq!(
            map_cluster_type("strict_dns"),
            cluster::DiscoveryType::StrictDns as i32
        );
        assert_eq!(
            map_cluster_type("logical_dns"),
            cluster::DiscoveryType::LogicalDns as i32
        );
        assert_eq!(
            map_cluster_type("unknown"),
            cluster::DiscoveryType::LogicalDns as i32
        );
    }
}
