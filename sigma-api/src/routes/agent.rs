use axum::{extract::State, routing::post, Json, Router};

use crate::errors::AppError;
use crate::models::{AgentHeartbeat, AgentRegister, IpEntry, Vps};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/agent/register", post(register))
        .route("/api/agent/heartbeat", post(heartbeat))
}

async fn register(
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

    let row = if let Some(existing) = existing {
        let mut extra = existing.extra.clone();
        if let serde_json::Value::Object(ref mut map) = extra {
            map.insert("system_info".to_string(), input.system_info.clone());
            map.insert(
                "last_heartbeat".to_string(),
                serde_json::Value::String(now),
            );
        }

        sqlx::query_as::<_, Vps>(
            r#"UPDATE vps SET
                ip_addresses = $2,
                ssh_port = $3,
                extra = $4,
                status = 'active'
               WHERE id = $1
               RETURNING *"#,
        )
        .bind(existing.id)
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
                extra, notes
            ) VALUES (
                $1, '', NULL,
                $2, $3,
                '', '', '',
                NULL, NULL, NULL, NULL,
                NULL, 'USD',
                'active', NULL, NULL,
                '', '', '{}',
                true, 9100,
                $4, ''
            ) RETURNING *"#,
        )
        .bind(&input.hostname)
        .bind(&ip_json)
        .bind(input.ssh_port)
        .bind(&extra)
        .fetch_one(&state.db)
        .await?
    };

    Ok(Json(row))
}

async fn heartbeat(
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
