use chrono::{DateTime, NaiveDate, Utc};
use ipnetwork::IpNetwork;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── Provider ────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
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

#[derive(Debug, Deserialize)]
pub struct CreateProvider {
    pub name: String,
    #[serde(default)]
    pub country: String,
    #[serde(default)]
    pub website: String,
    #[serde(default)]
    pub panel_url: String,
    #[serde(default)]
    pub api_supported: bool,
    pub rating: Option<i16>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProvider {
    pub name: Option<String>,
    pub country: Option<String>,
    pub website: Option<String>,
    pub panel_url: Option<String>,
    pub api_supported: Option<bool>,
    pub rating: Option<Option<i16>>,
    pub notes: Option<String>,
}

// ─── VPS ─────────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Vps {
    pub id: Uuid,
    pub hostname: String,
    pub alias: String,
    pub provider_id: Uuid,

    pub ip_addresses: Vec<IpNetwork>,
    pub ssh_port: i32,

    pub country: String,
    pub city: String,
    pub dc_name: String,

    pub cpu_cores: Option<i16>,
    pub ram_mb: Option<i32>,
    pub disk_gb: Option<i32>,
    pub bandwidth_tb: Option<Decimal>,

    pub cost_monthly: Option<Decimal>,
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

#[derive(Debug, Deserialize)]
pub struct CreateVps {
    pub hostname: String,
    #[serde(default)]
    pub alias: String,
    pub provider_id: Uuid,

    #[serde(default)]
    pub ip_addresses: Vec<String>, // accept as strings, parse to IpNetwork
    #[serde(default = "default_ssh_port")]
    pub ssh_port: i32,

    #[serde(default)]
    pub country: String,
    #[serde(default)]
    pub city: String,
    #[serde(default)]
    pub dc_name: String,

    pub cpu_cores: Option<i16>,
    pub ram_mb: Option<i32>,
    pub disk_gb: Option<i32>,
    pub bandwidth_tb: Option<f64>,

    pub cost_monthly: Option<f64>,
    #[serde(default = "default_currency")]
    pub currency: String,

    #[serde(default = "default_status")]
    pub status: String,
    pub purchase_date: Option<NaiveDate>,
    pub expire_date: Option<NaiveDate>,

    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub vpn_protocol: String,
    #[serde(default)]
    pub tags: Vec<String>,

    #[serde(default = "default_true")]
    pub monitoring_enabled: bool,
    #[serde(default = "default_node_exporter_port")]
    pub node_exporter_port: i32,

    #[serde(default)]
    pub extra: serde_json::Value,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateVps {
    pub hostname: Option<String>,
    pub alias: Option<String>,
    pub provider_id: Option<Uuid>,
    pub ip_addresses: Option<Vec<String>>,
    pub ssh_port: Option<i32>,
    pub country: Option<String>,
    pub city: Option<String>,
    pub dc_name: Option<String>,
    pub cpu_cores: Option<Option<i16>>,
    pub ram_mb: Option<Option<i32>>,
    pub disk_gb: Option<Option<i32>>,
    pub bandwidth_tb: Option<Option<f64>>,
    pub cost_monthly: Option<Option<f64>>,
    pub currency: Option<String>,
    pub status: Option<String>,
    pub purchase_date: Option<Option<NaiveDate>>,
    pub expire_date: Option<Option<NaiveDate>>,
    pub purpose: Option<String>,
    pub vpn_protocol: Option<String>,
    pub tags: Option<Vec<String>>,
    pub monitoring_enabled: Option<bool>,
    pub node_exporter_port: Option<i32>,
    pub extra: Option<serde_json::Value>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct VpsListQuery {
    pub status: Option<String>,
    pub country: Option<String>,
    pub provider_id: Option<Uuid>,
    pub purpose: Option<String>,
    pub tag: Option<String>,
    pub expiring_within_days: Option<i32>,
}

// ─── Prometheus target output ────────────────────────────

#[derive(Debug, Serialize)]
pub struct PrometheusTarget {
    pub targets: Vec<String>,
    pub labels: std::collections::HashMap<String, String>,
}

// ─── Stats ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct DashboardStats {
    pub total_vps: i64,
    pub active_vps: i64,
    pub total_providers: i64,
    pub by_country: Vec<CountStat>,
    pub by_provider: Vec<CountStat>,
    pub by_status: Vec<CountStat>,
    pub expiring_soon: Vec<Vps>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CountStat {
    pub label: Option<String>,
    pub count: Option<i64>,
}

// ─── Defaults ────────────────────────────────────────────

fn default_ssh_port() -> i32 { 22 }
fn default_currency() -> String { "USD".into() }
fn default_status() -> String { "provisioning".into() }
fn default_true() -> bool { true }
fn default_node_exporter_port() -> i32 { 9100 }
