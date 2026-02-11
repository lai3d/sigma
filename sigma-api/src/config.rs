pub struct Config {
    pub database_url: String,
    pub listen_host: String,
    pub listen_port: u16,
    pub db_max_conn: u32,
    pub api_key: Option<String>,
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
        }
    }
}
