use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpEntry {
    pub ip: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Vps {
    pub id: Uuid,
    pub ip_addresses: Vec<IpEntry>,
    pub ssh_port: i32,
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

#[derive(Debug, Serialize)]
pub struct CreateIpCheck {
    pub vps_id: Uuid,
    pub ip: String,
    pub check_type: Option<String>,
    pub source: Option<String>,
    pub success: bool,
    pub latency_ms: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct IpCheck {
    pub id: Uuid,
    pub vps_id: Uuid,
    pub ip: String,
    pub check_type: String,
    pub source: String,
    pub success: bool,
    pub latency_ms: Option<i32>,
    pub checked_at: DateTime<Utc>,
}
