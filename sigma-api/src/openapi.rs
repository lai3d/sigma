use utoipa::OpenApi;

use crate::errors::ErrorResponse;
#[allow(unused_imports)]
use crate::models::{
    AgentHeartbeat, AgentRegister, BatchCreateEnvoyRoutes, ChangePasswordRequest, ConvertedTotal, CostMonthlyResponse,
    CostSummaryResponse, CountStat, CreateEnvoyNode, CreateEnvoyRoute, CreateExchangeRate,
    CreateIpCheck, CreateProvider, CreateTicket, CreateTicketComment, CreateUser, CreateVps,
    CurrencyBreakdown, DashboardStats, EnvoyNode, EnvoyRoute, ExchangeRate, ImportRequest,
    ImportResult, IpCheck, IpCheckSummary, IpEntry, LoginRequest, LoginResponse, MonthlyCostEntry,
    PaginatedEnvoyNodeResponse, PaginatedEnvoyRouteResponse, PaginatedExchangeRateResponse,
    PaginatedIpCheckResponse, PaginatedProviderResponse, PaginatedTicketResponse,
    PaginatedUserResponse, PaginatedVpsResponse, PrometheusTarget, Provider, Ticket,
    TicketComment, TotpChallengeResponse, TotpDisableRequest, TotpLoginRequest,
    TotpSetupResponse, TotpVerifyRequest, UpdateEnvoyNode, UpdateEnvoyRoute, UpdateExchangeRate,
    UpdateProvider, UpdateTicket, UpdateUser, UpdateVps, UserResponse, Vps,
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Sigma API",
        version = "0.1.0",
        description = "VPS fleet management API for Sigma platform"
    ),
    paths(
        // Auth
        crate::routes::auth_routes::login,
        crate::routes::auth_routes::login_totp,
        crate::routes::auth_routes::me,
        crate::routes::auth_routes::refresh,
        crate::routes::auth_routes::change_password,
        crate::routes::auth_routes::totp_setup,
        crate::routes::auth_routes::totp_verify,
        crate::routes::auth_routes::totp_disable,
        // Users
        crate::routes::users::list,
        crate::routes::users::get_one,
        crate::routes::users::create,
        crate::routes::users::update,
        crate::routes::users::delete,
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
        // Tickets
        crate::routes::tickets::list,
        crate::routes::tickets::get_one,
        crate::routes::tickets::create,
        crate::routes::tickets::update,
        crate::routes::tickets::delete,
        crate::routes::tickets::list_comments,
        crate::routes::tickets::add_comment,
        // Envoy
        crate::routes::envoy::list_nodes,
        crate::routes::envoy::get_node,
        crate::routes::envoy::create_node,
        crate::routes::envoy::update_node,
        crate::routes::envoy::delete_node,
        crate::routes::envoy::list_routes,
        crate::routes::envoy::get_route,
        crate::routes::envoy::create_route,
        crate::routes::envoy::update_route,
        crate::routes::envoy::delete_route,
        crate::routes::envoy::batch_create_routes,
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
        UserResponse, CreateUser, UpdateUser, PaginatedUserResponse,
        LoginRequest, LoginResponse, ChangePasswordRequest,
        TotpSetupResponse, TotpVerifyRequest, TotpDisableRequest,
        TotpLoginRequest, TotpChallengeResponse,
        Ticket, CreateTicket, UpdateTicket, TicketComment, CreateTicketComment,
        PaginatedTicketResponse,
        EnvoyNode, CreateEnvoyNode, UpdateEnvoyNode, PaginatedEnvoyNodeResponse,
        EnvoyRoute, CreateEnvoyRoute, UpdateEnvoyRoute, PaginatedEnvoyRouteResponse,
        BatchCreateEnvoyRoutes,
    )),
    tags(
        (name = "Auth", description = "Authentication and session management"),
        (name = "Users", description = "User management (admin only)"),
        (name = "Providers", description = "Cloud provider management"),
        (name = "VPS", description = "VPS instance management"),
        (name = "IP Checks", description = "IP reachability tracking"),
        (name = "Stats", description = "Dashboard statistics"),
        (name = "Prometheus", description = "Prometheus integration"),
        (name = "Agent", description = "VPS agent registration and heartbeat"),
        (name = "Ansible", description = "Ansible dynamic inventory"),
        (name = "Exchange Rates", description = "Currency exchange rate management"),
        (name = "Costs", description = "Cost tracking and reporting"),
        (name = "Tickets", description = "Issue tracking and ticket management"),
        (name = "Envoy", description = "Envoy xDS control plane — nodes and routes"),
    )
)]
pub struct ApiDoc;
