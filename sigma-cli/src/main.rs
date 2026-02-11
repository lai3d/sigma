mod client;
mod commands;
mod config;
mod models;
mod output;

use anyhow::Result;
use chrono::NaiveDate;
use clap::{Parser, Subcommand};
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "sigma", about = "Sigma VPS fleet management CLI")]
struct Cli {
    /// API base URL (overrides config file and SIGMA_API_URL)
    #[arg(long, global = true)]
    api_url: Option<String>,

    /// API key (overrides config file and SIGMA_API_KEY)
    #[arg(long, global = true)]
    api_key: Option<String>,

    /// Output raw JSON instead of tables
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage providers
    Providers {
        #[command(subcommand)]
        command: ProviderCommands,
    },
    /// Manage VPS instances
    Vps {
        #[command(subcommand)]
        command: VpsCommands,
    },
    /// Show dashboard statistics
    Stats,
    /// Manage CLI configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
}

// ─── Provider subcommands ────────────────────────────────

#[derive(Subcommand)]
enum ProviderCommands {
    /// List all providers
    List {
        #[arg(long, default_value = "1")]
        page: i64,
        #[arg(long, default_value = "25")]
        per_page: i64,
    },
    /// Get a provider by ID
    Get { id: Uuid },
    /// Create a new provider
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        country: Option<String>,
        #[arg(long)]
        website: Option<String>,
        #[arg(long)]
        panel_url: Option<String>,
        #[arg(long)]
        api_supported: bool,
        #[arg(long)]
        rating: Option<i16>,
        #[arg(long)]
        notes: Option<String>,
    },
    /// Update an existing provider
    Update {
        id: Uuid,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        country: Option<String>,
        #[arg(long)]
        website: Option<String>,
        #[arg(long)]
        panel_url: Option<String>,
        #[arg(long)]
        api_supported: Option<bool>,
        #[arg(long)]
        rating: Option<i16>,
        #[arg(long)]
        notes: Option<String>,
    },
    /// Delete a provider
    Delete { id: Uuid },
    /// Export providers
    Export {
        #[arg(long, default_value = "json")]
        format: String,
        /// Output file (stdout if not specified)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Import providers from file
    Import {
        /// Input file path
        file: String,
        #[arg(long, default_value = "json")]
        format: String,
    },
}

// ─── VPS subcommands ─────────────────────────────────────

#[derive(Subcommand)]
enum VpsCommands {
    /// List VPS instances with optional filters
    List {
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        country: Option<String>,
        #[arg(long)]
        provider_id: Option<Uuid>,
        #[arg(long)]
        purpose: Option<String>,
        #[arg(long)]
        tag: Option<String>,
        /// Show VPS expiring within N days
        #[arg(long)]
        expiring: Option<i32>,
        #[arg(long, default_value = "1")]
        page: i64,
        #[arg(long, default_value = "25")]
        per_page: i64,
    },
    /// Get a VPS by ID
    Get { id: Uuid },
    /// Create a new VPS
    Create {
        #[arg(long)]
        hostname: String,
        #[arg(long)]
        provider_id: Uuid,
        #[arg(long)]
        alias: Option<String>,
        /// IP address (repeatable, format: IP or IP:label)
        #[arg(long)]
        ip: Vec<String>,
        #[arg(long)]
        ssh_port: Option<i32>,
        #[arg(long)]
        country: Option<String>,
        #[arg(long)]
        city: Option<String>,
        #[arg(long)]
        dc_name: Option<String>,
        #[arg(long)]
        cpu_cores: Option<i16>,
        #[arg(long)]
        ram_mb: Option<i32>,
        #[arg(long)]
        disk_gb: Option<i32>,
        #[arg(long)]
        bandwidth_tb: Option<f64>,
        #[arg(long)]
        cost_monthly: Option<f64>,
        #[arg(long)]
        currency: Option<String>,
        #[arg(long)]
        status: Option<String>,
        /// Purchase date (YYYY-MM-DD)
        #[arg(long)]
        purchase_date: Option<NaiveDate>,
        /// Expiry date (YYYY-MM-DD)
        #[arg(long)]
        expire_date: Option<NaiveDate>,
        #[arg(long)]
        purpose: Option<String>,
        #[arg(long)]
        vpn_protocol: Option<String>,
        /// Tag (repeatable)
        #[arg(long)]
        tag: Vec<String>,
        #[arg(long)]
        monitoring_enabled: Option<bool>,
        #[arg(long)]
        node_exporter_port: Option<i32>,
        #[arg(long)]
        notes: Option<String>,
    },
    /// Update an existing VPS
    Update {
        id: Uuid,
        #[arg(long)]
        hostname: Option<String>,
        #[arg(long)]
        alias: Option<String>,
        #[arg(long)]
        provider_id: Option<Uuid>,
        /// IP address (repeatable, format: IP or IP:label). Replaces all IPs.
        #[arg(long)]
        ip: Option<Vec<String>>,
        #[arg(long)]
        ssh_port: Option<i32>,
        #[arg(long)]
        country: Option<String>,
        #[arg(long)]
        city: Option<String>,
        #[arg(long)]
        dc_name: Option<String>,
        #[arg(long)]
        cpu_cores: Option<i16>,
        #[arg(long)]
        ram_mb: Option<i32>,
        #[arg(long)]
        disk_gb: Option<i32>,
        #[arg(long)]
        bandwidth_tb: Option<f64>,
        #[arg(long)]
        cost_monthly: Option<f64>,
        #[arg(long)]
        currency: Option<String>,
        #[arg(long)]
        status: Option<String>,
        /// Purchase date (YYYY-MM-DD)
        #[arg(long)]
        purchase_date: Option<NaiveDate>,
        /// Expiry date (YYYY-MM-DD)
        #[arg(long)]
        expire_date: Option<NaiveDate>,
        #[arg(long)]
        purpose: Option<String>,
        #[arg(long)]
        vpn_protocol: Option<String>,
        /// Tags (repeatable). Replaces all tags.
        #[arg(long)]
        tag: Option<Vec<String>>,
        #[arg(long)]
        monitoring_enabled: Option<bool>,
        #[arg(long)]
        node_exporter_port: Option<i32>,
        #[arg(long)]
        notes: Option<String>,
    },
    /// Delete a VPS
    Delete { id: Uuid },
    /// Retire a VPS (sets status=retired, disables monitoring)
    Retire { id: Uuid },
    /// Export VPS data
    Export {
        #[arg(long, default_value = "json")]
        format: String,
        /// Output file (stdout if not specified)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Import VPS from file
    Import {
        /// Input file path
        file: String,
        #[arg(long, default_value = "json")]
        format: String,
    },
}

// ─── Config subcommands ──────────────────────────────────

#[derive(Subcommand)]
enum ConfigCommands {
    /// Set the API URL
    SetUrl { url: String },
    /// Set the API key
    SetKey { key: String },
}

// ─── Main dispatch ───────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Config commands don't need an HTTP client
    if let Commands::Config { command } = &cli.command {
        return match command {
            ConfigCommands::SetUrl { url } => config::set_config_value("api_url", url),
            ConfigCommands::SetKey { key } => config::set_config_value("api_key", key),
        };
    }

    let cfg = config::Config::load(
        cli.api_url.as_deref(),
        cli.api_key.as_deref(),
    )?;
    let client = client::SigmaClient::new(&cfg)?;
    let json = cli.json;

    match cli.command {
        Commands::Providers { command } => match command {
            ProviderCommands::List { page, per_page } => {
                commands::providers::list(&client, page, per_page, json).await
            }
            ProviderCommands::Get { id } => {
                commands::providers::get(&client, id, json).await
            }
            ProviderCommands::Create {
                name,
                country,
                website,
                panel_url,
                api_supported,
                rating,
                notes,
            } => {
                commands::providers::create(
                    &client,
                    name,
                    country,
                    website,
                    panel_url,
                    api_supported,
                    rating,
                    notes,
                    json,
                )
                .await
            }
            ProviderCommands::Update {
                id,
                name,
                country,
                website,
                panel_url,
                api_supported,
                rating,
                notes,
            } => {
                commands::providers::update(
                    &client,
                    id,
                    name,
                    country,
                    website,
                    panel_url,
                    api_supported,
                    rating,
                    notes,
                    json,
                )
                .await
            }
            ProviderCommands::Delete { id } => {
                commands::providers::delete(&client, id).await
            }
            ProviderCommands::Export { format, output } => {
                commands::providers::export(&client, &format, output.as_deref()).await
            }
            ProviderCommands::Import { file, format } => {
                commands::providers::import(&client, &file, &format).await
            }
        },
        Commands::Vps { command } => match command {
            VpsCommands::List {
                status,
                country,
                provider_id,
                purpose,
                tag,
                expiring,
                page,
                per_page,
            } => {
                commands::vps::list(
                    &client,
                    status.as_deref(),
                    country.as_deref(),
                    provider_id,
                    purpose.as_deref(),
                    tag.as_deref(),
                    expiring,
                    page,
                    per_page,
                    json,
                )
                .await
            }
            VpsCommands::Get { id } => {
                commands::vps::get(&client, id, json).await
            }
            VpsCommands::Create {
                hostname,
                provider_id,
                alias,
                ip,
                ssh_port,
                country,
                city,
                dc_name,
                cpu_cores,
                ram_mb,
                disk_gb,
                bandwidth_tb,
                cost_monthly,
                currency,
                status,
                purchase_date,
                expire_date,
                purpose,
                vpn_protocol,
                tag,
                monitoring_enabled,
                node_exporter_port,
                notes,
            } => {
                commands::vps::create(
                    &client,
                    hostname,
                    provider_id,
                    alias,
                    ip,
                    ssh_port,
                    country,
                    city,
                    dc_name,
                    cpu_cores,
                    ram_mb,
                    disk_gb,
                    bandwidth_tb,
                    cost_monthly,
                    currency,
                    status,
                    purchase_date,
                    expire_date,
                    purpose,
                    vpn_protocol,
                    tag,
                    monitoring_enabled,
                    node_exporter_port,
                    notes,
                    json,
                )
                .await
            }
            VpsCommands::Update {
                id,
                hostname,
                alias,
                provider_id,
                ip,
                ssh_port,
                country,
                city,
                dc_name,
                cpu_cores,
                ram_mb,
                disk_gb,
                bandwidth_tb,
                cost_monthly,
                currency,
                status,
                purchase_date,
                expire_date,
                purpose,
                vpn_protocol,
                tag,
                monitoring_enabled,
                node_exporter_port,
                notes,
            } => {
                commands::vps::update(
                    &client,
                    id,
                    hostname,
                    alias,
                    provider_id,
                    ip,
                    ssh_port,
                    country,
                    city,
                    dc_name,
                    cpu_cores,
                    ram_mb,
                    disk_gb,
                    bandwidth_tb,
                    cost_monthly,
                    currency,
                    status,
                    purchase_date,
                    expire_date,
                    purpose,
                    vpn_protocol,
                    tag,
                    monitoring_enabled,
                    node_exporter_port,
                    notes,
                    json,
                )
                .await
            }
            VpsCommands::Delete { id } => {
                commands::vps::delete(&client, id).await
            }
            VpsCommands::Retire { id } => {
                commands::vps::retire(&client, id, json).await
            }
            VpsCommands::Export { format, output } => {
                commands::vps::export(&client, &format, output.as_deref()).await
            }
            VpsCommands::Import { file, format } => {
                commands::vps::import(&client, &file, &format).await
            }
        },
        Commands::Stats => commands::stats::stats(&client, json).await,
        Commands::Config { .. } => unreachable!(),
    }
}
