use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── IP Entry ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpEntry {
    pub ip: String,
    #[serde(default)]
    pub label: String,
}

// ─── Provider ────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct Provider {
    pub id: Uuid,
    pub name: String,
    pub country: String,
    pub website: String,
    pub panel_url: String,
    pub api_supported: bool,
    pub rating: Option<i16>,
    pub notes: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Default)]
pub struct CreateProvider {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub panel_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_supported: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Default)]
pub struct UpdateProvider {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub panel_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_supported: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<Option<i16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

// ─── VPS ─────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct Vps {
    pub id: Uuid,
    pub hostname: String,
    pub alias: String,
    pub provider_id: Uuid,
    pub ip_addresses: Vec<IpEntry>,
    pub ssh_port: i32,
    pub country: String,
    pub city: String,
    pub dc_name: String,
    pub cpu_cores: Option<i16>,
    pub ram_mb: Option<i32>,
    pub disk_gb: Option<i32>,
    pub bandwidth_tb: Option<f64>,
    pub cost_monthly: Option<f64>,
    pub currency: String,
    pub status: String,
    pub purchase_date: Option<NaiveDate>,
    pub expire_date: Option<NaiveDate>,
    pub purpose: String,
    pub vpn_protocol: String,
    pub tags: Vec<String>,
    pub monitoring_enabled: bool,
    pub node_exporter_port: i32,
    pub extra: serde_json::Value,
    pub notes: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Default)]
pub struct CreateVps {
    pub hostname: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    pub provider_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_addresses: Option<Vec<IpEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_port: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dc_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_cores: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ram_mb: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_gb: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bandwidth_tb: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_monthly: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purchase_date: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expire_date: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpn_protocol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monitoring_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_exporter_port: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Default)]
pub struct UpdateVps {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_addresses: Option<Vec<IpEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_port: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dc_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_cores: Option<Option<i16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ram_mb: Option<Option<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_gb: Option<Option<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bandwidth_tb: Option<Option<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_monthly: Option<Option<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purchase_date: Option<Option<NaiveDate>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expire_date: Option<Option<NaiveDate>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpn_protocol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monitoring_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_exporter_port: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

// ─── Pagination ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

// ─── Stats ───────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_vps: i64,
    pub active_vps: i64,
    pub total_providers: i64,
    pub by_country: Vec<CountStat>,
    pub by_provider: Vec<CountStat>,
    pub by_status: Vec<CountStat>,
    pub expiring_soon: Vec<Vps>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CountStat {
    pub label: Option<String>,
    pub count: Option<i64>,
}

// ─── IP Checks ───────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize)]
pub struct CreateIpCheck {
    pub vps_id: Uuid,
    pub ip: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IpCheckSummary {
    pub vps_id: Uuid,
    pub ip: String,
    pub total_checks: i64,
    pub success_count: i64,
    pub success_rate: f64,
    pub avg_latency_ms: Option<f64>,
    pub last_check: DateTime<Utc>,
    pub last_success: bool,
}

#[derive(Debug, Deserialize)]
pub struct PurgeResult {
    pub deleted: u64,
}

// ─── Import / Export ─────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ImportRequest {
    pub format: String,
    pub data: String,
}

#[derive(Debug, Deserialize)]
pub struct ImportResult {
    pub imported: usize,
    pub errors: Vec<String>,
}
