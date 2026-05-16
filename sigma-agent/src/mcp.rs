//! MCP-compatible JSON-RPC 2.0 server for sigma-agent.
//!
//! Exposes sigma-agent capabilities as LLM-callable tools so an external
//! controller (Claude / Doubao / any MCP client) can query fleet state
//! and allocate resources without each agent needing to embed an LLM.
//!
//! ## Resource-budget contract
//!
//! The agent runs on VPS instances as small as 1 vCPU / 512MB, alongside
//! the primary VPN workload. To keep within the agent's resource budget
//! (<1% steady-state CPU, <50MB RSS):
//!
//! - **No LLM in agent** — heavy reasoning happens centrally (sigma-api).
//! - **No persistent state** — every request is stateless; idle cost ≈ 0.
//! - **Reuses existing capabilities** — no new background loops; each tool
//!   wraps data already collected by port_scan / ebpf_traffic / xDS.
//! - **Network localhost by default** — bind defaults to 127.0.0.1:9103 so
//!   the surface is not internet-exposed without an explicit override.
//!
//! ## Protocol
//!
//! JSON-RPC 2.0 over HTTP POST at `/mcp`. Implements MCP methods:
//!   - `initialize` — protocol handshake.
//!   - `tools/list` — enumerate tools with JSON schemas.
//!   - `tools/call` — invoke a tool by name with arguments.
//!
//! Tools exposed:
//!   - `query_metrics` — host + port-scan snapshot
//!   - `query_ebpf_traffic` — per-process network observability (eBPF)
//!   - `allocate_ports` — get N free ports via real-time bind tests
//!   - `query_envoy_routes` — xDS-managed routes for this VPS
//!   - `query_dns_leaks` — processes sending UDP to port 53

use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::task::spawn_blocking;
use tracing::{error, info};

use crate::client::SigmaClient;
use crate::port_scan::{self, SharedScanResult};
use crate::system;

#[cfg(feature = "ebpf-traffic")]
use crate::ebpf_traffic::SharedTrafficStats;

const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
const JSONRPC_VERSION: &str = "2.0";

// JSON-RPC 2.0 error codes
const ERR_PARSE: i32 = -32700;
const ERR_METHOD_NOT_FOUND: i32 = -32601;
const ERR_INVALID_PARAMS: i32 = -32602;

// ---------- JSON-RPC envelope ----------

#[derive(Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcErrBody>,
}

#[derive(Serialize)]
struct JsonRpcErrBody {
    code: i32,
    message: String,
}

fn ok(id: Option<Value>, result: Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: JSONRPC_VERSION,
        id,
        result: Some(result),
        error: None,
    }
}

fn err(id: Option<Value>, code: i32, message: impl Into<String>) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: JSONRPC_VERSION,
        id,
        result: None,
        error: Some(JsonRpcErrBody {
            code,
            message: message.into(),
        }),
    }
}

// ---------- Shared MCP state ----------

pub struct McpState {
    pub scan_result: SharedScanResult,
    pub hostname: String,
    pub port_range: Option<(u16, u16)>,
    pub client: Arc<SigmaClient>,
    pub vps_id: Option<uuid::Uuid>,
    pub metrics_port: u16,
    pub public_ip: Option<String>,
    pub agent_version: &'static str,
    #[cfg(feature = "ebpf-traffic")]
    pub traffic_stats: Option<SharedTrafficStats>,
}

// ---------- Tool schemas (returned by tools/list) ----------

fn tools_list_response() -> Value {
    json!({
        "tools": [
            {
                "name": "query_metrics",
                "description": "Get current host system info (CPU, RAM, disk, load, uptime, public IP) and port-scan snapshot (total/available/in-use ports, breakdown by process category). Use this for health overview and capacity planning.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }
            },
            {
                "name": "query_ebpf_traffic",
                "description": "Query per-process network observability data collected via eBPF kprobes/tracepoints: TCP/UDP bytes, RTT, retransmits, connection counts, connection latency, packet drops by reason, DNS queries, exec events, OOM kills. Returns enabled=false if eBPF traffic monitoring is not configured on this agent.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "process": {
                            "type": "string",
                            "description": "Optional process-name filter (case-insensitive substring match)."
                        }
                    },
                    "additionalProperties": false
                }
            },
            {
                "name": "allocate_ports",
                "description": "Allocate N free TCP ports from the configured scan range via real-time bind tests. Stateless — caller must bind the returned ports immediately. Returns error if port scanning is disabled, or if fewer than N free ports are available.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "count": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 1000,
                            "description": "Number of ports to allocate (1-1000)."
                        }
                    },
                    "required": ["count"],
                    "additionalProperties": false
                }
            },
            {
                "name": "query_envoy_routes",
                "description": "Query Envoy routes for this VPS via the sigma-api. Returns list of routes (listen_port, backend host:port, cluster_type, source=dynamic|static). Requires the agent to have registered successfully (vps_id known).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "source": {
                            "type": "string",
                            "enum": ["dynamic", "static", "all"],
                            "description": "Filter routes by source (default all)."
                        }
                    },
                    "additionalProperties": false
                }
            },
            {
                "name": "query_dns_leaks",
                "description": "Detect potential DNS leaks: processes that emitted UDP packets to port 53 in the last eBPF harvest window. On a VPN node, any process bypassing the tunnel for DNS resolution is a security concern. Returns enabled=false if eBPF traffic monitoring is not configured.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "min_queries": {
                            "type": "integer",
                            "minimum": 1,
                            "description": "Minimum DNS query count threshold (default 1)."
                        }
                    },
                    "additionalProperties": false
                }
            }
        ]
    })
}

fn tool_text(text: String) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false
    })
}

fn tool_err(text: String) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": true
    })
}

// ---------- Tool dispatch ----------

async fn handle_tool_call(state: Arc<McpState>, params: Value) -> Value {
    let name = match params.get("name").and_then(|v| v.as_str()) {
        Some(n) => n,
        None => return tool_err("missing 'name' in tool call".to_string()),
    };
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or(Value::Object(Default::default()));

    let result = match name {
        "query_metrics" => tool_query_metrics(&state).await,
        "query_ebpf_traffic" => tool_query_ebpf_traffic(&state, args).await,
        "allocate_ports" => tool_allocate_ports(&state, args).await,
        "query_envoy_routes" => tool_query_envoy_routes(&state, args).await,
        "query_dns_leaks" => tool_query_dns_leaks(&state, args).await,
        other => return tool_err(format!("unknown tool: {}", other)),
    };

    match result {
        Ok(text) => tool_text(text),
        Err(msg) => tool_err(msg),
    }
}

// ---------- Individual tools ----------

async fn tool_query_metrics(state: &McpState) -> Result<String, String> {
    let scan = state.scan_result.read().await.clone();
    let in_use = scan.total_ports.saturating_sub(scan.available);

    let system_info =
        system::collect_system_info(state.metrics_port, state.public_ip.as_deref());

    let payload = json!({
        "hostname": state.hostname,
        "agent_version": state.agent_version,
        "system": system_info,
        "ports": {
            "total_in_range": scan.total_ports,
            "available": scan.available,
            "in_use": in_use,
            "by_source": scan.used_by_source,
            "other_detail": scan.other_detail,
            "last_scan_duration_secs": scan.scan_duration.as_secs_f64(),
        }
    });

    serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())
}

#[cfg(feature = "ebpf-traffic")]
async fn tool_query_ebpf_traffic(state: &McpState, args: Value) -> Result<String, String> {
    let Some(ref stats_arc) = state.traffic_stats else {
        return Ok(json!({"enabled": false, "stats": []}).to_string());
    };

    let filter = args
        .get("process")
        .and_then(|v| v.as_str())
        .map(|s| s.to_ascii_lowercase());

    let stats = stats_arc.read().await;
    let filtered: Vec<Value> = stats
        .iter()
        .filter(|t| match &filter {
            Some(f) => t.process_name.to_ascii_lowercase().contains(f),
            None => true,
        })
        .map(|t| {
            json!({
                "process": t.process_name,
                "container": t.container_id,
                "tcp_bytes_sent": t.bytes_sent,
                "tcp_bytes_recv": t.bytes_recv,
                "udp_bytes_sent": t.udp_bytes_sent,
                "udp_bytes_recv": t.udp_bytes_recv,
                "retransmits": t.retransmits,
                "active_connections": t.active_connections,
                "total_connections": t.total_connections,
                "rtt_us": { "avg": t.rtt_avg_us, "min": t.rtt_min_us, "max": t.rtt_max_us },
                "connection_latency_us": {
                    "avg": t.conn_latency_avg_us,
                    "min": t.conn_latency_min_us,
                    "max": t.conn_latency_max_us
                },
                "drops": t.drops.iter().map(|(r, c)| json!({"reason": r, "count": c})).collect::<Vec<_>>(),
                "dns_queries": t.dns_queries,
                "dns_bytes": t.dns_bytes,
                "exec_count": t.exec_count,
                "oom_kills": t.oom_kills,
            })
        })
        .collect();

    serde_json::to_string_pretty(&json!({
        "enabled": true,
        "count": filtered.len(),
        "stats": filtered
    }))
    .map_err(|e| e.to_string())
}

#[cfg(not(feature = "ebpf-traffic"))]
async fn tool_query_ebpf_traffic(_state: &McpState, _args: Value) -> Result<String, String> {
    Ok(json!({
        "enabled": false,
        "stats": [],
        "note": "agent built without ebpf-traffic feature"
    })
    .to_string())
}

async fn tool_allocate_ports(state: &McpState, args: Value) -> Result<String, String> {
    let count = args
        .get("count")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| "'count' is required and must be an integer".to_string())?
        as usize;

    if count == 0 || count > 1000 {
        return Err("'count' must be between 1 and 1000".to_string());
    }

    let (start, end) = state
        .port_range
        .ok_or_else(|| "port scanning is not enabled on this agent".to_string())?;

    let ports = spawn_blocking(move || port_scan::find_available_ports(start, end, count))
        .await
        .map_err(|e| format!("scan task failed: {}", e))?;

    if ports.len() < count {
        return Err(format!(
            "only {} ports available in range {}-{}, requested {}",
            ports.len(),
            start,
            end,
            count
        ));
    }

    let payload = json!({
        "allocated": ports,
        "count": ports.len(),
        "range": format!("{}-{}", start, end),
    });
    Ok(payload.to_string())
}

async fn tool_query_envoy_routes(state: &McpState, args: Value) -> Result<String, String> {
    use crate::models::{EnvoyNode, EnvoyRoute, PaginatedResponse};

    let vps_id = state
        .vps_id
        .ok_or_else(|| "agent has not registered with sigma-api (vps_id unknown)".to_string())?;

    let source_filter = args
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("all");
    let source_query = match source_filter {
        "dynamic" => "&source=dynamic",
        "static" => "&source=static",
        _ => "",
    };

    let nodes: PaginatedResponse<EnvoyNode> = state
        .client
        .get(&format!(
            "/envoy-nodes?vps_id={}&status=active&per_page=100",
            vps_id
        ))
        .await
        .map_err(|e| format!("fetch envoy_nodes failed: {:#}", e))?;

    let mut all_routes = Vec::new();
    for node in &nodes.data {
        let routes: PaginatedResponse<EnvoyRoute> = state
            .client
            .get(&format!(
                "/envoy-routes?envoy_node_id={}&status=active{}&per_page=1000",
                node.id, source_query
            ))
            .await
            .map_err(|e| format!("fetch envoy_routes failed: {:#}", e))?;

        for r in routes.data {
            all_routes.push(json!({
                "node_id": node.node_id,
                "name": r.name,
                "listen_port": r.listen_port,
                "backend": format!(
                    "{}:{}",
                    r.backend_host.as_deref().unwrap_or("-"),
                    r.backend_port.unwrap_or(0)
                ),
                "cluster_type": r.cluster_type,
                "source": r.source,
            }));
        }
    }

    serde_json::to_string_pretty(&json!({
        "count": all_routes.len(),
        "routes": all_routes,
        "source_filter": source_filter,
    }))
    .map_err(|e| e.to_string())
}

#[cfg(feature = "ebpf-traffic")]
async fn tool_query_dns_leaks(state: &McpState, args: Value) -> Result<String, String> {
    let Some(ref stats_arc) = state.traffic_stats else {
        return Ok(json!({"enabled": false, "leaks": []}).to_string());
    };

    let min = args
        .get("min_queries")
        .and_then(|v| v.as_u64())
        .unwrap_or(1);

    let stats = stats_arc.read().await;
    let leaks: Vec<Value> = stats
        .iter()
        .filter(|t| t.dns_queries >= min)
        .map(|t| {
            json!({
                "process": t.process_name,
                "container": t.container_id,
                "dns_queries": t.dns_queries,
                "dns_bytes": t.dns_bytes,
            })
        })
        .collect();

    serde_json::to_string_pretty(&json!({
        "enabled": true,
        "min_queries": min,
        "count": leaks.len(),
        "leaks": leaks,
    }))
    .map_err(|e| e.to_string())
}

#[cfg(not(feature = "ebpf-traffic"))]
async fn tool_query_dns_leaks(_state: &McpState, _args: Value) -> Result<String, String> {
    Ok(json!({
        "enabled": false,
        "leaks": [],
        "note": "agent built without ebpf-traffic feature"
    })
    .to_string())
}

// ---------- HTTP handler ----------

async fn mcp_handler(
    State(state): State<Arc<McpState>>,
    body: axum::body::Bytes,
) -> Json<JsonRpcResponse> {
    let req: JsonRpcRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return Json(err(None, ERR_PARSE, format!("parse error: {}", e))),
    };

    let id = req.id.clone();

    let response = match req.method.as_str() {
        "initialize" => ok(
            id,
            json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "sigma-agent",
                    "version": state.agent_version,
                }
            }),
        ),
        "tools/list" => ok(id, tools_list_response()),
        "tools/call" => {
            let result = handle_tool_call(state.clone(), req.params).await;
            ok(id, result)
        }
        // Notifications carry no id; client doesn't expect a response.
        // Returning an empty ok is harmless for clients that ignore it.
        "notifications/initialized" => ok(id, Value::Null),
        method => err(
            id,
            ERR_METHOD_NOT_FOUND,
            format!("method not found: {}", method),
        ),
    };

    Json(response)
}

#[allow(dead_code)] // referenced in error reporting via ERR_INVALID_PARAMS
const _: i32 = ERR_INVALID_PARAMS;

pub async fn serve_mcp(bind: String, state: Arc<McpState>) {
    let app = Router::new()
        .route("/mcp", post(mcp_handler))
        .with_state(state);

    let listener = match TcpListener::bind(&bind).await {
        Ok(l) => l,
        Err(e) => {
            error!(bind = %bind, error = %e, "Failed to bind MCP server");
            return;
        }
    };

    info!(bind = %bind, "MCP server listening on /mcp (JSON-RPC 2.0, MCP protocol)");

    if let Err(e) = axum::serve(listener, app).await {
        error!(error = %e, "MCP server error");
    }
}
