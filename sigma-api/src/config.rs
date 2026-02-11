pub struct Config {
    pub database_url: String,
    pub listen_host: String,
    pub listen_port: u16,
    pub db_max_conn: u32,
    pub api_key: Option<String>,
    pub redis_url: String,
    pub rate_limit_requests: u32,
    pub rate_limit_window: u64,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://sigma:sigma@localhost/sigma".into()),
            listen_host: std::env::var("LISTEN_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            listen_port: std::env::var("LISTEN_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            db_max_conn: std::env::var("DB_MAX_CONN")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(10),
            api_key: std::env::var("API_KEY").ok(),
            redis_url: std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379".into()),
            rate_limit_requests: std::env::var("RATE_LIMIT_REQUESTS")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(100),
            rate_limit_window: std::env::var("RATE_LIMIT_WINDOW")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(60),
        }
    }
}
