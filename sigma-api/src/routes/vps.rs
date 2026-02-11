use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{CreateVps, IpEntry, UpdateVps, Vps, VpsListQuery};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/vps", get(list).post(create))
        .route("/api/vps/{id}", get(get_one).put(update).delete(delete))
        .route("/api/vps/{id}/retire", axum::routing::post(retire))
}

async fn list(
    State(state): State<AppState>,
    Query(q): Query<VpsListQuery>,
) -> Result<Json<Vec<Vps>>, AppError> {
    let mut sql = String::from(
        "SELECT * FROM vps WHERE 1=1"
    );
    let mut param_idx = 0u32;

    if q.status.is_some() {
        param_idx += 1;
        sql.push_str(&format!(" AND status = ${}", param_idx));
    }
    if q.country.is_some() {
        param_idx += 1;
        sql.push_str(&format!(" AND country = ${}", param_idx));
    }
    if q.provider_id.is_some() {
        param_idx += 1;
        sql.push_str(&format!(" AND provider_id = ${}", param_idx));
    }
    if q.purpose.is_some() {
        param_idx += 1;
        sql.push_str(&format!(" AND purpose = ${}", param_idx));
    }
    if q.tag.is_some() {
        param_idx += 1;
        sql.push_str(&format!(" AND ${} = ANY(tags)", param_idx));
    }
    if q.expiring_within_days.is_some() {
        param_idx += 1;
        sql.push_str(&format!(
            " AND expire_date IS NOT NULL AND expire_date <= CURRENT_DATE + (${} || ' days')::INTERVAL",
            param_idx
        ));
    }

    sql.push_str(" ORDER BY status, expire_date ASC NULLS LAST, hostname");

    let mut query = sqlx::query_as::<_, Vps>(&sql);

    if let Some(ref v) = q.status {
        query = query.bind(v);
    }
    if let Some(ref v) = q.country {
        query = query.bind(v);
    }
    if let Some(ref v) = q.provider_id {
        query = query.bind(v);
    }
    if let Some(ref v) = q.purpose {
        query = query.bind(v);
    }
    if let Some(ref v) = q.tag {
        query = query.bind(v);
    }
    if let Some(v) = q.expiring_within_days {
        query = query.bind(v);
    }

    let rows = query.fetch_all(&state.db).await?;
    Ok(Json(rows))
}

async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vps>, AppError> {
    let row = sqlx::query_as::<_, Vps>("SELECT * FROM vps WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

fn validate_ips(entries: &[IpEntry]) -> Result<(), AppError> {
    for e in entries {
        let ip_str = e.ip.trim();
        if ip_str.is_empty() {
            continue;
        }
        // Validate it's a parseable IP (v4 or v6)
        if ip_str.parse::<std::net::IpAddr>().is_err() {
            return Err(AppError::BadRequest(format!("Invalid IP address: '{}'", ip_str)));
        }
    }
    Ok(())
}

async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateVps>,
) -> Result<Json<Vps>, AppError> {
    validate_ips(&input.ip_addresses)?;

    let ip_json = serde_json::to_value(&input.ip_addresses)
        .map_err(|e| AppError::BadRequest(format!("Invalid ip_addresses: {}", e)))?;

    let row = sqlx::query_as::<_, Vps>(
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
            $1, $2, $3,
            $4, $5,
            $6, $7, $8,
            $9, $10, $11, $12,
            $13, $14,
            $15, $16, $17,
            $18, $19, $20,
            $21, $22,
            $23, $24
        ) RETURNING *"#,
    )
    .bind(&input.hostname)
    .bind(&input.alias)
    .bind(input.provider_id)
    .bind(&ip_json)
    .bind(input.ssh_port)
    .bind(&input.country)
    .bind(&input.city)
    .bind(&input.dc_name)
    .bind(input.cpu_cores)
    .bind(input.ram_mb)
    .bind(input.disk_gb)
    .bind(input.bandwidth_tb.and_then(Decimal::from_f64_retain))
    .bind(input.cost_monthly.and_then(Decimal::from_f64_retain))
    .bind(&input.currency)
    .bind(&input.status)
    .bind(input.purchase_date)
    .bind(input.expire_date)
    .bind(&input.purpose)
    .bind(&input.vpn_protocol)
    .bind(&input.tags)
    .bind(input.monitoring_enabled)
    .bind(input.node_exporter_port)
    .bind(&input.extra)
    .bind(&input.notes)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(row))
}

async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateVps>,
) -> Result<Json<Vps>, AppError> {
    let existing = sqlx::query_as::<_, Vps>("SELECT * FROM vps WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let ip_json = match input.ip_addresses {
        Some(ref addrs) => {
            validate_ips(addrs)?;
            serde_json::to_value(addrs)
                .map_err(|e| AppError::BadRequest(format!("Invalid ip_addresses: {}", e)))?
        }
        None => serde_json::to_value(&existing.ip_addresses.0)
            .unwrap_or_default(),
    };

    let row = sqlx::query_as::<_, Vps>(
        r#"UPDATE vps SET
            hostname = $2, alias = $3, provider_id = $4,
            ip_addresses = $5, ssh_port = $6,
            country = $7, city = $8, dc_name = $9,
            cpu_cores = $10, ram_mb = $11, disk_gb = $12, bandwidth_tb = $13,
            cost_monthly = $14, currency = $15,
            status = $16, purchase_date = $17, expire_date = $18,
            purpose = $19, vpn_protocol = $20, tags = $21,
            monitoring_enabled = $22, node_exporter_port = $23,
            extra = $24, notes = $25
           WHERE id = $1
           RETURNING *"#,
    )
    .bind(id)
    .bind(input.hostname.unwrap_or(existing.hostname))
    .bind(input.alias.unwrap_or(existing.alias))
    .bind(input.provider_id.unwrap_or(existing.provider_id))
    .bind(&ip_json)
    .bind(input.ssh_port.unwrap_or(existing.ssh_port))
    .bind(input.country.unwrap_or(existing.country))
    .bind(input.city.unwrap_or(existing.city))
    .bind(input.dc_name.unwrap_or(existing.dc_name))
    .bind(input.cpu_cores.unwrap_or(existing.cpu_cores))
    .bind(input.ram_mb.unwrap_or(existing.ram_mb))
    .bind(input.disk_gb.unwrap_or(existing.disk_gb))
    .bind(input.bandwidth_tb.map(|v| v.and_then(Decimal::from_f64_retain)).unwrap_or(existing.bandwidth_tb))
    .bind(input.cost_monthly.map(|v| v.and_then(Decimal::from_f64_retain)).unwrap_or(existing.cost_monthly))
    .bind(input.currency.unwrap_or(existing.currency))
    .bind(input.status.unwrap_or(existing.status))
    .bind(input.purchase_date.unwrap_or(existing.purchase_date))
    .bind(input.expire_date.unwrap_or(existing.expire_date))
    .bind(input.purpose.unwrap_or(existing.purpose))
    .bind(input.vpn_protocol.unwrap_or(existing.vpn_protocol))
    .bind(input.tags.unwrap_or(existing.tags))
    .bind(input.monitoring_enabled.unwrap_or(existing.monitoring_enabled))
    .bind(input.node_exporter_port.unwrap_or(existing.node_exporter_port))
    .bind(input.extra.unwrap_or(existing.extra))
    .bind(input.notes.unwrap_or(existing.notes))
    .fetch_one(&state.db)
    .await?;

    Ok(Json(row))
}

async fn delete(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let result = sqlx::query("DELETE FROM vps WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(serde_json::json!({ "deleted": true })))
}

/// Quick action: mark a VPS as retired
async fn retire(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vps>, AppError> {
    let row = sqlx::query_as::<_, Vps>(
        "UPDATE vps SET status = 'retired', monitoring_enabled = false WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(row))
}
