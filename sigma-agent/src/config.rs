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

    /// SSH port to report
    #[arg(long, env = "AGENT_SSH_PORT", default_value = "22")]
    pub ssh_port: i32,
}
