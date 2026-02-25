use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "sigma-agent", about = "System agent for sigma VPS fleet management")]
pub struct Config {
    /// Sigma API base URL
    #[arg(long, env = "SIGMA_API_URL", default_value = "http://localhost:3000/api")]
    pub api_url: String,

    /// API key for authentication
    #[arg(long, env = "SIGMA_API_KEY")]
    pub api_key: Option<String>,

    /// Heartbeat interval in seconds
    #[arg(long, env = "AGENT_INTERVAL", default_value = "60")]
    pub interval: u64,

    /// Override auto-detected hostname
    #[arg(long, env = "AGENT_HOSTNAME")]
    pub hostname: Option<String>,

    /// Alias for this VPS (e.g. vn001.example.com)
    #[arg(long, env = "AGENT_ALIAS")]
    pub alias: Option<String>,

    /// SSH port to report
    #[arg(long, env = "AGENT_SSH_PORT", default_value = "22")]
    pub ssh_port: i32,

    /// Prometheus metrics server port (0 to disable)
    #[arg(long, env = "AGENT_METRICS_PORT", default_value = "9102")]
    pub metrics_port: u16,

    /// Enable port scanning
    #[arg(long, env = "AGENT_PORT_SCAN", default_value = "false")]
    pub port_scan: bool,

    /// Port scan range (START-END)
    #[arg(long, env = "AGENT_PORT_SCAN_RANGE", default_value = "10000-30000")]
    pub port_scan_range: String,

    /// Port scan interval in seconds
    #[arg(long, env = "AGENT_PORT_SCAN_INTERVAL", default_value = "60")]
    pub port_scan_interval: u64,

    /// Enable xDS gRPC server
    #[arg(long, env = "AGENT_XDS_ENABLED", default_value = "false")]
    pub xds_enabled: bool,

    /// xDS gRPC listen port
    #[arg(long, env = "AGENT_XDS_PORT", default_value = "18000")]
    pub xds_port: u16,

    /// xDS config poll interval in seconds
    #[arg(long, env = "AGENT_XDS_POLL_INTERVAL", default_value = "10")]
    pub xds_poll_interval: u64,

    /// Path to Envoy static config file
    #[arg(long, env = "AGENT_ENVOY_CONFIG_PATH", default_value = "/etc/envoy/envoy.yaml")]
    pub envoy_config_path: String,

    /// Enable static config sync (parse envoy.yaml and POST to API)
    #[arg(long, env = "AGENT_ENVOY_CONFIG_SYNC", default_value = "false")]
    pub envoy_config_sync: bool,

    /// Envoy config file poll interval in seconds
    #[arg(long, env = "AGENT_ENVOY_CONFIG_SYNC_INTERVAL", default_value = "60")]
    pub envoy_config_sync_interval: u64,
}

impl Config {
    pub fn parse_port_scan_range(&self) -> anyhow::Result<(u16, u16)> {
        let parts: Vec<&str> = self.port_scan_range.split('-').collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid port scan range format, expected START-END: {}", self.port_scan_range);
        }
        let start: u16 = parts[0].parse().map_err(|_| anyhow::anyhow!("Invalid start port: {}", parts[0]))?;
        let end: u16 = parts[1].parse().map_err(|_| anyhow::anyhow!("Invalid end port: {}", parts[1]))?;
        if start > end {
            anyhow::bail!("Start port {} is greater than end port {}", start, end);
        }
        Ok((start, end))
    }
}
