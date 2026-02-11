mod config;
mod db;
mod errors;
mod models;
mod notifications;
mod routes;

use axum::Router;
use sqlx::postgres::PgPoolOptions;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let cfg = config::Config::from_env();

    let pool = PgPoolOptions::new()
        .max_connections(cfg.db_max_conn)
        .connect(&cfg.database_url)
        .await?;

    tracing::info!("Connected to database");

    // Run migrations
    sqlx::migrate::Migrator::new(std::path::Path::new("./migrations"))
        .await?
        .run(&pool)
        .await?;
    tracing::info!("Migrations applied");

    // Connect to Redis
    let redis_client = redis::Client::open(cfg.redis_url.as_str())?;
    let redis_conn = redis::aio::ConnectionManager::new(redis_client).await?;
    tracing::info!("Connected to Redis");

    let http_client = reqwest::Client::new();
    let addr = format!("{}:{}", cfg.listen_host, cfg.listen_port);

    let app_state = routes::AppState {
        db: pool,
        api_key: cfg.api_key.clone(),
        redis: redis_conn,
        rate_limit_requests: cfg.rate_limit_requests,
        rate_limit_window: cfg.rate_limit_window,
        http_client: http_client.clone(),
    };

    // Spawn notification worker if any channel is configured
    if cfg.telegram_bot_token.is_some() || cfg.webhook_url.is_some() {
        let notify_db = app_state.db.clone();
        let notify_redis = app_state.redis.clone();
        tokio::spawn(notifications::run(notify_db, notify_redis, http_client, cfg));
    } else {
        tracing::info!("Notification worker disabled (no TELEGRAM_BOT_TOKEN or WEBHOOK_URL set)");
    }

    let app = Router::new()
        .merge(routes::providers::router())
        .merge(routes::vps::router())
        .merge(routes::ip_checks::router())
        .merge(routes::prometheus::router())
        .merge(routes::stats::router())
        .merge(routes::agent::router())
        .merge(routes::ansible::router())
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            routes::rate_limit::rate_limit,
        ))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            routes::auth,
        ))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(app_state);
    tracing::info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>())
        .await?;

    Ok(())
}
