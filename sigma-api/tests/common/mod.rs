#[allow(unused_imports)]
use axum::{body::Body, http::Request, Router};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;

use sigma_api::auth;
use sigma_api::routes::{self, AppState};

const ADMIN_EMAIL: &str = "admin@test.local";
const ADMIN_PASSWORD: &str = "testpass123";

pub async fn setup() -> (Router, PgPool) {
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    // Run migrations
    sqlx::migrate::Migrator::new(std::path::Path::new("./migrations"))
        .await
        .expect("Failed to load migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    // Clean all tables before each test suite
    cleanup(&pool).await;

    // Seed admin user
    let password_hash = auth::hash_password(ADMIN_PASSWORD).expect("Failed to hash password");
    sqlx::query(
        "INSERT INTO users (email, password_hash, name, role, force_password_change) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(ADMIN_EMAIL)
    .bind(&password_hash)
    .bind("Test Admin")
    .bind("admin")
    .bind(false)
    .execute(&pool)
    .await
    .expect("Failed to seed admin user");

    // Connect to Redis
    let redis_client =
        redis::Client::open(redis_url.as_str()).expect("Failed to create Redis client");
    let redis_conn = redis::aio::ConnectionManager::new(redis_client)
        .await
        .expect("Failed to connect to Redis");

    let app_state = AppState {
        db: pool.clone(),
        api_key: Some("test-api-key".to_string()),
        redis: redis_conn,
        rate_limit_requests: 1000,
        rate_limit_window: 60,
        http_client: reqwest::Client::new(),
        jwt_secret: "test-jwt-secret".to_string(),
        jwt_expiry_hours: 24,
    };

    // Build router matching main.rs structure
    let public_routes = routes::auth_routes::router();

    let api_routes = Router::new()
        .merge(routes::providers::router())
        .merge(routes::vps::router())
        .merge(routes::ip_checks::router())
        .merge(routes::prometheus::router())
        .merge(routes::stats::router())
        .merge(routes::agent::router())
        .merge(routes::ansible::router())
        .merge(routes::exchange_rates::router())
        .merge(routes::costs::router())
        .merge(routes::auth_routes::protected_router())
        .merge(routes::users::router())
        .merge(routes::audit_logs::router())
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            routes::rate_limit::rate_limit,
        ))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            routes::auth,
        ));

    let router = Router::new()
        .merge(public_routes)
        .merge(api_routes)
        .with_state(app_state);

    (router, pool)
}

pub async fn login_admin(router: &Router) -> String {
    let body = serde_json::json!({
        "email": ADMIN_EMAIL,
        "password": ADMIN_PASSWORD,
    });

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200, "Admin login should succeed");

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    json["token"].as_str().unwrap().to_string()
}

/// Send a request with a JWT Bearer token and return (status, body_json)
pub async fn request_with_token(
    router: &Router,
    method: &str,
    uri: &str,
    token: &str,
    body: Option<Value>,
) -> (u16, Value) {
    let body = match body {
        Some(json) => Body::from(serde_json::to_string(&json).unwrap()),
        None => Body::empty(),
    };

    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {}", token))
        .header("content-type", "application/json")
        .body(body)
        .unwrap();

    let response = router.clone().oneshot(req).await.unwrap();
    let status = response.status().as_u16();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();

    let json: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };

    (status, json)
}

/// Send a request with X-Api-Key header
#[allow(dead_code)]
pub async fn request_with_api_key(
    router: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (u16, Value) {
    let body = match body {
        Some(json) => Body::from(serde_json::to_string(&json).unwrap()),
        None => Body::empty(),
    };

    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("x-api-key", "test-api-key")
        .header("content-type", "application/json")
        .body(body)
        .unwrap();

    let response = router.clone().oneshot(req).await.unwrap();
    let status = response.status().as_u16();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();

    let json: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };

    (status, json)
}

pub async fn cleanup(pool: &PgPool) {
    // Truncate all tables in dependency order
    sqlx::query(
        "TRUNCATE TABLE audit_logs, ip_checks, vps, providers, exchange_rates, users RESTART IDENTITY CASCADE",
    )
    .execute(pool)
    .await
    .expect("Failed to truncate tables");
}
