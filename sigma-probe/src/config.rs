use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "sigma-probe", about = "IP reachability probe for sigma VPS fleet")]
pub struct Config {
    /// Sigma API base URL
    #[arg(long, env = "SIGMA_API_URL", default_value = "http://localhost:3000/api")]
    pub api_url: String,

    /// API key for authentication
    #[arg(long, env = "SIGMA_API_KEY")]
    pub api_key: Option<String>,

    /// Probe source identifier (e.g. cn-beijing, cn-shanghai)
    #[arg(long, env = "PROBE_SOURCE")]
    pub source: String,

    /// Probe interval in seconds
    #[arg(long, env = "PROBE_INTERVAL", default_value = "300")]
    pub interval: u64,

    /// Comma-separated check types: icmp, tcp, http
    #[arg(long, env = "PROBE_TYPES", default_value = "icmp,tcp")]
    pub check_types: String,

    /// TCP connect timeout in seconds
    #[arg(long, env = "PROBE_TCP_TIMEOUT", default_value = "5")]
    pub tcp_timeout: u64,

    /// HTTP request timeout in seconds
    #[arg(long, env = "PROBE_HTTP_TIMEOUT", default_value = "10")]
    pub http_timeout: u64,

    /// Maximum concurrent checks
    #[arg(long, env = "PROBE_CONCURRENCY", default_value = "20")]
    pub concurrency: usize,
}

impl Config {
    pub fn check_types(&self) -> Vec<String> {
        self.check_types
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    }
}
