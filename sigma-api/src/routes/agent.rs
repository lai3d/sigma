use axum::{extract::State, routing::post, Json, Router};

use crate::errors::{AppError, ErrorResponse};
use crate::models::{AgentHeartbeat, AgentRegister, IpEntry, Vps};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/agent/register", post(register))
        .route("/api/agent/heartbeat", post(heartbeat))
}

#[utoipa::path(
    post, path = "/api/agent/register",
    tag = "Agent",
    request_body = AgentRegister,
    responses(
        (status = 200, body = Vps, description = "Registered or updated VPS"),
        (status = 400, body = ErrorResponse),
    )
)]
pub async fn register(
    State(state): State<AppState>,
    Json(input): Json<AgentRegister>,
) -> Result<Json<Vps>, AppError> {
    if input.hostname.trim().is_empty() {
        return Err(AppError::BadRequest("hostname is required".into()));
    }

    validate_ips(&input.ip_addresses)?;

    let ip_json = serde_json::to_value(&input.ip_addresses)
        .map_err(|e| AppError::BadRequest(format!("Invalid ip_addresses: {}", e)))?;

    let existing = sqlx::query_as::<_, Vps>("SELECT * FROM vps WHERE hostname = $1")
        .bind(&input.hostname)
        .fetch_optional(&state.db)
        .await?;

    let now = chrono::Utc::now().to_rfc3339();

    let alias = input.alias.as_deref().unwrap_or("");

    let row = if let Some(existing) = existing {
        let mut extra = existing.extra.clone();
        if let serde_json::Value::Object(ref mut map) = extra {
            map.insert("system_info".to_string(), input.system_info.clone());
            map.insert(
                "last_heartbeat".to_string(),
                serde_json::Value::String(now),
            );
        }

        // Only update alias if agent provides one; keep existing otherwise
        let effective_alias = if alias.is_empty() {
            &existing.alias
        } else {
            alias
        };

        sqlx::query_as::<_, Vps>(
            r#"UPDATE vps SET
                alias = $2,
                ip_addresses = $3,
                ssh_port = $4,
                extra = $5,
                status = 'active'
               WHERE id = $1
               RETURNING *"#,
        )
        .bind(existing.id)
        .bind(effective_alias)
        .bind(&ip_json)
        .bind(input.ssh_port)
        .bind(&extra)
        .fetch_one(&state.db)
        .await?
    } else {
        let extra = serde_json::json!({
            "system_info": input.system_info,
            "last_heartbeat": now,
        });

        sqlx::query_as::<_, Vps>(
            r#"INSERT INTO vps (
                hostname, alias, provider_id,
                ip_addresses, ssh_port,
                country, city, dc_name,
                cpu_cores, ram_mb, disk_gb, bandwidth_tb,
                cost_monthly, currency,
                status, purchase_date, expire_date,
                purpose, vpn_protocol, tags,
                monitoring_enabled, node_exporter_port,
                extra, notes,
                source
            ) VALUES (
                $1, $2, NULL,
                $3, $4,
                '', '', '',
                NULL, NULL, NULL, NULL,
                NULL, 'USD',
                'active', NULL, NULL,
                '', '', '{}',
                true, 9100,
                $5, '',
                'agent'
            ) RETURNING *"#,
        )
        .bind(&input.hostname)
        .bind(alias)
        .bind(&ip_json)
        .bind(input.ssh_port)
        .bind(&extra)
        .fetch_one(&state.db)
        .await?
    };

    Ok(Json(row))
}

#[utoipa::path(
    post, path = "/api/agent/heartbeat",
    tag = "Agent",
    request_body = AgentHeartbeat,
    responses(
        (status = 200, body = Vps, description = "Updated VPS"),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
    )
)]
pub async fn heartbeat(
    State(state): State<AppState>,
    Json(input): Json<AgentHeartbeat>,
) -> Result<Json<Vps>, AppError> {
    if input.hostname.trim().is_empty() {
        return Err(AppError::BadRequest("hostname is required".into()));
    }

    let existing = sqlx::query_as::<_, Vps>("SELECT * FROM vps WHERE hostname = $1")
        .bind(&input.hostname)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let mut extra = existing.extra.clone();
    if let serde_json::Value::Object(ref mut map) = extra {
        map.insert("system_info".to_string(), input.system_info.clone());
        map.insert(
            "last_heartbeat".to_string(),
            serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
        );
    }

    let row = sqlx::query_as::<_, Vps>("UPDATE vps SET extra = $2 WHERE id = $1 RETURNING *")
        .bind(existing.id)
        .bind(&extra)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(row))
}

fn validate_ips(entries: &[IpEntry]) -> Result<(), AppError> {
    for e in entries {
        let ip_str = e.ip.trim();
        if ip_str.is_empty() {
            continue;
        }
        if ip_str.parse::<std::net::IpAddr>().is_err() {
            return Err(AppError::BadRequest(format!(
                "Invalid IP address: '{}'",
                ip_str
            )));
        }
    }
    Ok(())
}
