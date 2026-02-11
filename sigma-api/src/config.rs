pub struct Config {
    pub database_url: String,
    pub listen_host: String,
    pub listen_port: u16,
    pub db_max_conn: u32,
    pub api_key: Option<String>,
    pub redis_url: String,
    pub rate_limit_requests: u32,
    pub rate_limit_window: u64,
    pub telegram_bot_token: Option<String>,
    pub telegram_chat_id: Option<String>,
    pub webhook_url: Option<String>,
    pub notify_before_days: Vec<i32>,
    pub notify_interval_secs: u64,
    pub jwt_secret: String,
    pub jwt_expiry_hours: u64,
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
            telegram_bot_token: std::env::var("TELEGRAM_BOT_TOKEN")
                .ok()
                .filter(|s| !s.is_empty()),
            telegram_chat_id: std::env::var("TELEGRAM_CHAT_ID")
                .ok()
                .filter(|s| !s.is_empty()),
            webhook_url: std::env::var("WEBHOOK_URL")
                .ok()
                .filter(|s| !s.is_empty()),
            notify_before_days: std::env::var("NOTIFY_BEFORE_DAYS")
                .unwrap_or_else(|_| "7,3,1".into())
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect(),
            notify_interval_secs: std::env::var("NOTIFY_INTERVAL_SECS")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3600),
            jwt_secret: std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| "sigma-default-jwt-secret-change-me".into()),
            jwt_expiry_hours: std::env::var("JWT_EXPIRY_HOURS")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(24),
        }
    }
}
