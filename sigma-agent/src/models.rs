use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpEntry {
    pub ip: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Serialize)]
pub struct AgentRegister {
    pub hostname: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    pub ip_addresses: Vec<IpEntry>,
    pub ssh_port: i32,
    pub system_info: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct AgentHeartbeat {
    pub hostname: String,
    pub system_info: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct VpsResponse {
    pub id: uuid::Uuid,
    pub hostname: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct EnvoyNode {
    pub id: uuid::Uuid,
    pub vps_id: uuid::Uuid,
    pub node_id: String,
    pub admin_port: Option<i32>,
    pub description: String,
    pub config_version: i64,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreateEnvoyNode {
    pub vps_id: uuid::Uuid,
    pub node_id: String,
    pub description: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct EnvoyRoute {
    pub id: uuid::Uuid,
    pub envoy_node_id: uuid::Uuid,
    pub name: String,
    pub listen_port: i32,
    pub backend_host: Option<String>,
    pub backend_port: Option<i32>,
    pub cluster_type: String,
    pub connect_timeout_secs: i32,
    pub proxy_protocol: i32,
    pub source: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct SyncStaticRoutesRequest {
    pub envoy_node_id: uuid::Uuid,
    pub routes: Vec<StaticRouteEntry>,
}

#[derive(Debug, Serialize)]
pub struct StaticRouteEntry {
    pub name: String,
    pub listen_port: i32,
    pub backend_host: Option<String>,
    pub backend_port: Option<i32>,
    pub cluster_type: String,
    pub connect_timeout_secs: i32,
    pub proxy_protocol: i32,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SyncStaticRoutesResponse {
    pub upserted: usize,
    pub deleted: usize,
}
