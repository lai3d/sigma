use utoipa::OpenApi;

use crate::errors::ErrorResponse;
#[allow(unused_imports)]
use crate::models::{
    AgentHeartbeat, AgentRegister, ConvertedTotal, CostMonthlyResponse, CostSummaryResponse,
    CountStat, CreateExchangeRate, CreateIpCheck, CreateProvider, CreateVps, CurrencyBreakdown,
    DashboardStats, ExchangeRate, ImportRequest, ImportResult, IpCheck, IpCheckSummary, IpEntry,
    MonthlyCostEntry, PaginatedExchangeRateResponse, PaginatedIpCheckResponse,
    PaginatedProviderResponse, PaginatedVpsResponse, PrometheusTarget, Provider,
    UpdateExchangeRate, UpdateProvider, UpdateVps, Vps,
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Sigma API",
        version = "0.1.0",
        description = "VPS fleet management API for Sigma platform"
    ),
    paths(
        // Providers
        crate::routes::providers::list,
        crate::routes::providers::get_one,
        crate::routes::providers::create,
        crate::routes::providers::update,
        crate::routes::providers::delete,
        crate::routes::providers::export,
        crate::routes::providers::import,
        // VPS
        crate::routes::vps::list,
        crate::routes::vps::get_one,
        crate::routes::vps::create,
        crate::routes::vps::update,
        crate::routes::vps::delete,
        crate::routes::vps::retire,
        crate::routes::vps::export,
        crate::routes::vps::import,
        // IP Checks
        crate::routes::ip_checks::list,
        crate::routes::ip_checks::get_one,
        crate::routes::ip_checks::create,
        crate::routes::ip_checks::delete,
        crate::routes::ip_checks::summary,
        crate::routes::ip_checks::purge,
        // Stats
        crate::routes::stats::dashboard,
        // Prometheus
        crate::routes::prometheus::targets,
        // Agent
        crate::routes::agent::register,
        crate::routes::agent::heartbeat,
        // Ansible
        crate::routes::ansible::inventory,
        // Exchange Rates
        crate::routes::exchange_rates::list,
        crate::routes::exchange_rates::get_one,
        crate::routes::exchange_rates::create,
        crate::routes::exchange_rates::update,
        crate::routes::exchange_rates::delete,
        // Costs
        crate::routes::costs::summary,
        crate::routes::costs::monthly,
    ),
    components(schemas(
        ErrorResponse,
        IpEntry,
        Provider, CreateProvider, UpdateProvider,
        Vps, CreateVps, UpdateVps,
        PaginatedProviderResponse, PaginatedVpsResponse, PaginatedIpCheckResponse,
        PrometheusTarget,
        DashboardStats, CountStat,
        ImportRequest, ImportResult,
        IpCheck, CreateIpCheck, IpCheckSummary,
        AgentRegister, AgentHeartbeat,
        ExchangeRate, CreateExchangeRate, UpdateExchangeRate, PaginatedExchangeRateResponse,
        CurrencyBreakdown, ConvertedTotal, CostSummaryResponse,
        MonthlyCostEntry, CostMonthlyResponse,
    )),
    tags(
        (name = "Providers", description = "Cloud provider management"),
        (name = "VPS", description = "VPS instance management"),
        (name = "IP Checks", description = "IP reachability tracking"),
        (name = "Stats", description = "Dashboard statistics"),
        (name = "Prometheus", description = "Prometheus integration"),
        (name = "Agent", description = "VPS agent registration and heartbeat"),
        (name = "Ansible", description = "Ansible dynamic inventory"),
        (name = "Exchange Rates", description = "Currency exchange rate management"),
        (name = "Costs", description = "Cost tracking and reporting"),
    )
)]
pub struct ApiDoc;
