use axum::{
    extract::{ConnectInfo, Request, State},
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use redis::AsyncCommands;
use std::net::SocketAddr;

use super::AppState;

/// Rate limiting middleware using Redis sliding window counters.
/// Limits requests per client IP within a configurable time window.
pub async fn rate_limit(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let ip = extract_client_ip(&req);
    let key = format!("rate:{ip}");
    let limit = state.rate_limit_requests;
    let window = state.rate_limit_window;

    let mut conn = state.redis.clone();

    // INCR the counter; if it's the first request in the window, set TTL
    let count: u32 = match redis::cmd("INCR").arg(&key).query_async(&mut conn).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Redis INCR error: {e}");
            // Fail open: allow request if Redis is down
            return Ok(next.run(req).await);
        }
    };

    if count == 1 {
        // First request in window — set expiry
        if let Err(e) = conn.expire::<_, ()>(&key, window as i64).await {
            tracing::error!("Redis EXPIRE error: {e}");
        }
    }

    if count > limit {
        // Fetch TTL for Retry-After header
        let ttl: i64 = conn.ttl(&key).await.unwrap_or(window as i64);
        let mut response = Response::new(axum::body::Body::from("Too Many Requests"));
        *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
        set_rate_headers(response.headers_mut(), limit, 0, ttl);
        return Ok(response);
    }

    let mut response = next.run(req).await;
    let remaining = limit.saturating_sub(count);
    set_rate_headers(response.headers_mut(), limit, remaining, 0);
    Ok(response)
}

fn extract_client_ip(req: &Request) -> String {
    // Prefer X-Forwarded-For (first IP) for reverse-proxy setups
    if let Some(forwarded) = req.headers().get("x-forwarded-for") {
        if let Ok(val) = forwarded.to_str() {
            if let Some(first) = val.split(',').next() {
                let trimmed = first.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }

    // Fall back to ConnectInfo from request extensions
    if let Some(ConnectInfo(addr)) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
        return addr.ip().to_string();
    }

    "unknown".to_string()
}

fn set_rate_headers(
    headers: &mut axum::http::HeaderMap,
    limit: u32,
    remaining: u32,
    retry_after: i64,
) {
    if let Ok(v) = HeaderValue::from_str(&limit.to_string()) {
        headers.insert("X-RateLimit-Limit", v);
    }
    if let Ok(v) = HeaderValue::from_str(&remaining.to_string()) {
        headers.insert("X-RateLimit-Remaining", v);
    }
    if retry_after > 0 {
        if let Ok(v) = HeaderValue::from_str(&retry_after.to_string()) {
            headers.insert("Retry-After", v);
        }
    }
}
