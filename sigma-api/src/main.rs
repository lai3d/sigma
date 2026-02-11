mod config;
mod db;
mod errors;
mod models;
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

    let app_state = routes::AppState {
        db: pool,
        api_key: cfg.api_key.clone(),
    };

    let app = Router::new()
        .merge(routes::providers::router())
        .merge(routes::vps::router())
        .merge(routes::ip_checks::router())
        .merge(routes::prometheus::router())
        .merge(routes::stats::router())
        .merge(routes::agent::router())
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    let addr = format!("{}:{}", cfg.listen_host, cfg.listen_port);
    tracing::info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
