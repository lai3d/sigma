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
