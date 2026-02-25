use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

// ─── IP Entry with label ────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct IpEntry {
    pub ip: String,
    #[serde(default)]
    pub label: String,
}

// ─── Provider ────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
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

#[derive(Debug, Deserialize, ToSchema)]
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

#[derive(Debug, Deserialize, ToSchema)]
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

#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct Vps {
    pub id: Uuid,
    pub hostname: String,
    pub alias: String,
    pub provider_id: Option<Uuid>,

    #[schema(value_type = Vec<IpEntry>)]
    pub ip_addresses: sqlx::types::Json<Vec<IpEntry>>,
    pub ssh_port: i32,

    pub country: String,
    pub city: String,
    pub dc_name: String,

    pub cpu_cores: Option<i16>,
    pub ram_mb: Option<i32>,
    pub disk_gb: Option<i32>,
    #[schema(value_type = Option<String>)]
    pub bandwidth_tb: Option<Decimal>,

    #[schema(value_type = Option<String>)]
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

    #[schema(value_type = Object)]
    pub extra: serde_json::Value,
    pub notes: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateVps {
    pub hostname: String,
    #[serde(default)]
    pub alias: String,
    #[serde(default)]
    pub provider_id: Option<Uuid>,

    #[serde(default)]
    pub ip_addresses: Vec<IpEntry>,
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
    #[schema(value_type = Object)]
    pub extra: serde_json::Value,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateVps {
    pub hostname: Option<String>,
    pub alias: Option<String>,
    pub provider_id: Option<Uuid>,
    pub ip_addresses: Option<Vec<IpEntry>>,
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
    #[schema(value_type = Option<Object>)]
    pub extra: Option<serde_json::Value>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct VpsListQuery {
    pub status: Option<String>,
    pub country: Option<String>,
    pub provider_id: Option<Uuid>,
    pub purpose: Option<String>,
    pub tag: Option<String>,
    pub expiring_within_days: Option<i32>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

// ─── Pagination ──────────────────────────────────────────

#[derive(Debug, Deserialize, IntoParams)]
pub struct ProviderListQuery {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub data: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

// Concrete paginated response types for OpenAPI schema generation
#[derive(Serialize, ToSchema)]
pub struct PaginatedProviderResponse {
    pub data: Vec<Provider>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Serialize, ToSchema)]
pub struct PaginatedVpsResponse {
    pub data: Vec<Vps>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Serialize, ToSchema)]
pub struct PaginatedIpCheckResponse {
    pub data: Vec<IpCheck>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

fn default_page() -> i64 { 1 }
fn default_per_page() -> i64 { 25 }

// ─── Prometheus target output ────────────────────────────

#[derive(Debug, Serialize, ToSchema)]
pub struct PrometheusTarget {
    pub targets: Vec<String>,
    pub labels: std::collections::HashMap<String, String>,
}

// ─── Stats ───────────────────────────────────────────────

#[derive(Debug, Serialize, ToSchema)]
pub struct DashboardStats {
    pub total_vps: i64,
    pub active_vps: i64,
    pub total_providers: i64,
    pub by_country: Vec<CountStat>,
    pub by_provider: Vec<CountStat>,
    pub by_status: Vec<CountStat>,
    pub expiring_soon: Vec<Vps>,
}

#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct CountStat {
    pub label: Option<String>,
    pub count: Option<i64>,
}

// ─── Import / Export ─────────────────────────────────────

#[derive(Debug, Deserialize, ToSchema)]
pub struct ImportRequest {
    /// Format: "csv" or "json"
    pub format: String,
    /// Raw CSV or JSON data as a string
    pub data: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ImportResult {
    pub imported: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ProviderCsvRow {
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

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct VpsCsvRow {
    pub hostname: String,
    #[serde(default)]
    pub alias: String,
    #[serde(default)]
    pub provider_name: String,
    #[serde(default)]
    pub ip_addresses: String,
    #[serde(default = "default_ssh_port_csv")]
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
    #[serde(default)]
    pub purchase_date: String,
    #[serde(default)]
    pub expire_date: String,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub vpn_protocol: String,
    #[serde(default)]
    pub tags: String,
    #[serde(default = "default_true")]
    pub monitoring_enabled: bool,
    #[serde(default = "default_node_exporter_port_csv")]
    pub node_exporter_port: i32,
    #[serde(default)]
    pub extra: String,
    #[serde(default)]
    pub notes: String,
}

fn default_ssh_port_csv() -> i32 { 22 }
fn default_node_exporter_port_csv() -> i32 { 9100 }

#[derive(Debug, Deserialize, IntoParams)]
pub struct ExportQuery {
    /// Format: "csv" or "json" (default: "json")
    #[serde(default = "default_json_format")]
    pub format: String,
}

fn default_json_format() -> String { "json".into() }

// ─── IP Checks ───────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
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

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateIpCheck {
    pub vps_id: Uuid,
    pub ip: String,
    #[serde(default = "default_icmp")]
    pub check_type: String,
    #[serde(default)]
    pub source: String,
    pub success: bool,
    pub latency_ms: Option<i32>,
}

fn default_icmp() -> String { "icmp".into() }

#[derive(Debug, Deserialize, IntoParams)]
pub struct IpCheckListQuery {
    pub vps_id: Option<Uuid>,
    pub ip: Option<String>,
    pub source: Option<String>,
    pub check_type: Option<String>,
    pub success: Option<bool>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
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

#[derive(Debug, Deserialize, IntoParams)]
pub struct IpCheckSummaryQuery {
    pub vps_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct PurgeQuery {
    pub older_than_days: i32,
}

// ─── Agent ───────────────────────────────────────────────

#[derive(Debug, Deserialize, ToSchema)]
pub struct AgentRegister {
    pub hostname: String,
    pub alias: Option<String>,
    #[serde(default)]
    pub ip_addresses: Vec<IpEntry>,
    #[serde(default = "default_ssh_port")]
    pub ssh_port: i32,
    #[serde(default)]
    #[schema(value_type = Object)]
    pub system_info: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AgentHeartbeat {
    pub hostname: String,
    #[serde(default)]
    #[schema(value_type = Object)]
    pub system_info: serde_json::Value,
}

// ─── Exchange Rates ──────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct ExchangeRate {
    pub id: Uuid,
    pub from_currency: String,
    pub to_currency: String,
    #[schema(value_type = String)]
    pub rate: Decimal,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateExchangeRate {
    pub from_currency: String,
    pub to_currency: String,
    pub rate: f64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateExchangeRate {
    pub rate: f64,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ExchangeRateListQuery {
    pub from_currency: Option<String>,
    pub to_currency: Option<String>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

#[derive(Serialize, ToSchema)]
pub struct PaginatedExchangeRateResponse {
    pub data: Vec<ExchangeRate>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

// ─── Cost Reporting ──────────────────────────────────────

#[derive(Debug, Deserialize, IntoParams)]
pub struct CostSummaryQuery {
    pub provider_id: Option<Uuid>,
    pub country: Option<String>,
    pub status: Option<String>,
    pub convert_to: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct CostSummaryRow {
    pub currency: String,
    pub vps_count: i64,
    pub total_cost: Decimal,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CurrencyBreakdown {
    pub currency: String,
    pub vps_count: i64,
    #[schema(value_type = String)]
    pub total_cost: Decimal,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConvertedTotal {
    pub currency: String,
    #[schema(value_type = String)]
    pub amount: Decimal,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CostSummaryResponse {
    pub per_currency: Vec<CurrencyBreakdown>,
    pub converted_total: Option<ConvertedTotal>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct CostMonthlyQuery {
    /// Number of months to look back (default 12)
    pub months: Option<i32>,
    pub provider_id: Option<Uuid>,
    pub country: Option<String>,
    pub convert_to: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct MonthlyCostRow {
    pub month: NaiveDate,
    pub currency: Option<String>,
    pub total_cost: Decimal,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MonthlyCostEntry {
    pub month: NaiveDate,
    pub per_currency: Vec<CurrencyBreakdown>,
    pub converted_total: Option<ConvertedTotal>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CostMonthlyResponse {
    pub months: Vec<MonthlyCostEntry>,
}

// ─── Users & Auth ────────────────────────────────────────

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub name: String,
    pub role: String,
    pub force_password_change: bool,
    #[serde(skip_serializing)]
    pub totp_secret: Option<String>,
    pub totp_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub role: String,
    pub force_password_change: bool,
    pub totp_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<User> for UserResponse {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            email: u.email,
            name: u.name,
            role: u.role,
            force_password_change: u.force_password_change,
            totp_enabled: u.totp_enabled,
            created_at: u.created_at,
            updated_at: u.updated_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUser {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_role")]
    pub role: String,
}

fn default_role() -> String { "readonly".into() }

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateUser {
    pub email: Option<String>,
    pub name: Option<String>,
    pub role: Option<String>,
    pub password: Option<String>,
    pub force_password_change: Option<bool>,
    pub totp_enabled: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TotpSetupResponse {
    pub secret: String,
    pub otpauth_url: String,
    pub qr_code: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TotpVerifyRequest {
    pub code: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TotpDisableRequest {
    pub code: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TotpLoginRequest {
    pub totp_token: String,
    pub code: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TotpChallengeResponse {
    pub requires_totp: bool,
    pub totp_token: String,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct UserListQuery {
    pub role: Option<String>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

#[derive(Serialize, ToSchema)]
pub struct PaginatedUserResponse {
    pub data: Vec<UserResponse>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

// ─── Tickets ─────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct Ticket {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub vps_id: Option<Uuid>,
    pub provider_id: Option<Uuid>,
    pub created_by: Uuid,
    pub assigned_to: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTicket {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_priority")]
    pub priority: String,
    pub vps_id: Option<Uuid>,
    pub provider_id: Option<Uuid>,
    pub assigned_to: Option<Uuid>,
}

fn default_priority() -> String { "medium".into() }

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateTicket {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub vps_id: Option<Option<Uuid>>,
    pub provider_id: Option<Option<Uuid>>,
    pub assigned_to: Option<Option<Uuid>>,
}

#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct TicketComment {
    pub id: Uuid,
    pub ticket_id: Uuid,
    pub user_id: Uuid,
    pub user_email: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTicketComment {
    pub body: String,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct TicketListQuery {
    pub status: Option<String>,
    pub priority: Option<String>,
    pub assigned_to: Option<Uuid>,
    pub vps_id: Option<Uuid>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

#[derive(Serialize, ToSchema)]
pub struct PaginatedTicketResponse {
    pub data: Vec<Ticket>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

// ─── Envoy Nodes ─────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct EnvoyNode {
    pub id: Uuid,
    pub vps_id: Uuid,
    pub node_id: String,
    pub admin_port: Option<i32>,
    pub description: String,
    pub config_version: i64,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateEnvoyNode {
    pub vps_id: Uuid,
    pub node_id: String,
    pub admin_port: Option<i32>,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_active")]
    pub status: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateEnvoyNode {
    pub node_id: Option<String>,
    pub admin_port: Option<Option<i32>>,
    pub description: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct EnvoyNodeListQuery {
    pub vps_id: Option<Uuid>,
    pub status: Option<String>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

#[derive(Serialize, ToSchema)]
pub struct PaginatedEnvoyNodeResponse {
    pub data: Vec<EnvoyNode>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

// ─── Envoy Routes ────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct EnvoyRoute {
    pub id: Uuid,
    pub envoy_node_id: Uuid,
    pub name: String,
    pub listen_port: i32,
    pub backend_host: Option<String>,
    pub backend_port: Option<i32>,
    pub cluster_type: String,
    pub connect_timeout_secs: i32,
    pub proxy_protocol: i32,
    pub source: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateEnvoyRoute {
    pub envoy_node_id: Uuid,
    pub name: String,
    pub listen_port: i32,
    pub backend_host: Option<String>,
    pub backend_port: Option<i32>,
    #[serde(default = "default_cluster_type")]
    pub cluster_type: String,
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_secs: i32,
    #[serde(default = "default_proxy_protocol")]
    pub proxy_protocol: i32,
    #[serde(default = "default_active")]
    pub status: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateEnvoyRoute {
    pub name: Option<String>,
    pub listen_port: Option<i32>,
    pub backend_host: Option<Option<String>>,
    pub backend_port: Option<Option<i32>>,
    pub cluster_type: Option<String>,
    pub connect_timeout_secs: Option<i32>,
    pub proxy_protocol: Option<i32>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct EnvoyRouteListQuery {
    pub envoy_node_id: Option<Uuid>,
    pub status: Option<String>,
    pub source: Option<String>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

#[derive(Serialize, ToSchema)]
pub struct PaginatedEnvoyRouteResponse {
    pub data: Vec<EnvoyRoute>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

// ─── Static Route Sync ───────────────────────────────

#[derive(Debug, Deserialize, ToSchema)]
pub struct SyncStaticRoutes {
    pub envoy_node_id: Uuid,
    pub routes: Vec<StaticRouteEntry>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct StaticRouteEntry {
    pub name: String,
    pub listen_port: i32,
    pub backend_host: Option<String>,
    pub backend_port: Option<i32>,
    #[serde(default = "default_cluster_type")]
    pub cluster_type: String,
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_secs: i32,
    #[serde(default)]
    pub proxy_protocol: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SyncStaticRoutesResponse {
    pub upserted: usize,
    pub deleted: usize,
}

// ─── Batch Operations ────────────────────────────────────

#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchCreateEnvoyRoutes {
    pub routes: Vec<CreateEnvoyRoute>,
}

// ─── Envoy Topology ─────────────────────────────────────

#[derive(Debug, Serialize, ToSchema)]
pub struct TopologyNode {
    pub id: Uuid,
    pub hostname: String,
    pub alias: String,
    pub country: String,
    pub purpose: String,
    pub status: String,
    #[schema(value_type = Vec<IpEntry>)]
    pub ip_addresses: Vec<IpEntry>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TopologyRouteInfo {
    pub name: String,
    pub listen_port: i32,
    pub backend_host: Option<String>,
    pub backend_port: Option<i32>,
    pub proxy_protocol: i32,
    pub source: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TopologyEdge {
    pub source_vps_id: Uuid,
    pub target_vps_id: Option<Uuid>,
    pub target_external: Option<String>,
    pub routes: Vec<TopologyRouteInfo>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TopologyResponse {
    pub nodes: Vec<TopologyNode>,
    pub edges: Vec<TopologyEdge>,
}

fn default_active() -> String { "active".into() }
fn default_cluster_type() -> String { "logical_dns".into() }
fn default_connect_timeout() -> i32 { 5 }
fn default_proxy_protocol() -> i32 { 1 }

// ─── Defaults ────────────────────────────────────────────

fn default_ssh_port() -> i32 { 22 }
fn default_currency() -> String { "USD".into() }
fn default_status() -> String { "provisioning".into() }
fn default_true() -> bool { true }
fn default_node_exporter_port() -> i32 { 9100 }

// ─── DNS (multi-provider) ────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct DnsAccount {
    pub id: Uuid,
    pub name: String,
    pub provider_type: String,
    #[serde(skip_serializing)]
    pub config: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DnsAccountResponse {
    pub id: Uuid,
    pub name: String,
    pub provider_type: String,
    pub masked_config: serde_json::Value,
    pub zones_count: i64,
    pub records_count: i64,
    pub last_synced: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateDnsAccount {
    pub name: String,
    pub provider_type: String,
    pub config: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateDnsAccount {
    pub name: Option<String>,
    pub config: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct DnsZone {
    pub id: Uuid,
    pub account_id: Uuid,
    pub zone_id: String,
    pub zone_name: String,
    pub status: String,
    pub domain_expires_at: Option<DateTime<Utc>>,
    pub cert_expires_at: Option<DateTime<Utc>>,
    pub synced_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct DnsZoneListQuery {
    pub account_id: Option<Uuid>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct DnsRecord {
    pub id: Uuid,
    pub zone_uuid: Uuid,
    pub record_id: String,
    pub record_type: String,
    pub name: String,
    pub content: String,
    pub ttl: i32,
    pub extra: serde_json::Value,
    pub vps_id: Option<Uuid>,
    pub synced_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // joined fields
    pub zone_name: Option<String>,
    pub zone_id_ext: Option<String>,
    pub vps_hostname: Option<String>,
    pub vps_country: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct DnsRecordListQuery {
    pub account_id: Option<Uuid>,
    pub zone_name: Option<String>,
    pub record_type: Option<String>,
    pub has_vps: Option<bool>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DnsSyncResult {
    pub zones_count: i64,
    pub records_count: i64,
    pub records_linked: i64,
    pub records_deleted: i64,
}

#[derive(Serialize, ToSchema)]
pub struct PaginatedDnsAccountResponse {
    pub data: Vec<DnsAccountResponse>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Serialize, ToSchema)]
pub struct PaginatedDnsZoneResponse {
    pub data: Vec<DnsZone>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Serialize, ToSchema)]
pub struct PaginatedDnsRecordResponse {
    pub data: Vec<DnsRecord>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct DnsAccountListQuery {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}
