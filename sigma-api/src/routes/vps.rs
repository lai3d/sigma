use axum::{
    extract::{Path, Query, State},
    http::header,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use rust_decimal::Decimal;
use std::collections::HashMap;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{
    CreateVps, ExportQuery, ImportRequest, ImportResult, IpEntry, PaginatedResponse, UpdateVps,
    Vps, VpsCsvRow, VpsListQuery,
};
use crate::routes::AppState;

const VPS_INSERT_SQL: &str = r#"INSERT INTO vps (
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
) RETURNING *"#;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/vps", get(list).post(create))
        .route("/api/vps/export", get(export))
        .route("/api/vps/import", axum::routing::post(import))
        .route("/api/vps/{id}", get(get_one).put(update).delete(delete))
        .route("/api/vps/{id}/retire", axum::routing::post(retire))
}

async fn list(
    State(state): State<AppState>,
    Query(q): Query<VpsListQuery>,
) -> Result<Json<PaginatedResponse<Vps>>, AppError> {
    let per_page = q.per_page.clamp(1, 100);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let mut where_clause = String::from(" WHERE 1=1");
    let mut param_idx = 0u32;

    if q.status.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND status = ${}", param_idx));
    }
    if q.country.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND country = ${}", param_idx));
    }
    if q.provider_id.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND provider_id = ${}", param_idx));
    }
    if q.purpose.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND purpose = ${}", param_idx));
    }
    if q.tag.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND ${} = ANY(tags)", param_idx));
    }
    if q.expiring_within_days.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(
            " AND expire_date IS NOT NULL AND expire_date <= CURRENT_DATE + (${} || ' days')::INTERVAL",
            param_idx
        ));
    }

    // Count query
    let count_sql = format!("SELECT COUNT(*) FROM vps{}", where_clause);
    let mut count_query = sqlx::query_as::<_, (i64,)>(&count_sql);

    if let Some(ref v) = q.status { count_query = count_query.bind(v); }
    if let Some(ref v) = q.country { count_query = count_query.bind(v); }
    if let Some(ref v) = q.provider_id { count_query = count_query.bind(v); }
    if let Some(ref v) = q.purpose { count_query = count_query.bind(v); }
    if let Some(ref v) = q.tag { count_query = count_query.bind(v); }
    if let Some(v) = q.expiring_within_days { count_query = count_query.bind(v); }

    let total = count_query.fetch_one(&state.db).await?.0;

    // Data query with pagination
    param_idx += 1;
    let limit_param = param_idx;
    param_idx += 1;
    let offset_param = param_idx;

    let data_sql = format!(
        "SELECT * FROM vps{} ORDER BY status, expire_date ASC NULLS LAST, hostname LIMIT ${} OFFSET ${}",
        where_clause, limit_param, offset_param
    );
    let mut query = sqlx::query_as::<_, Vps>(&data_sql);

    if let Some(ref v) = q.status { query = query.bind(v); }
    if let Some(ref v) = q.country { query = query.bind(v); }
    if let Some(ref v) = q.provider_id { query = query.bind(v); }
    if let Some(ref v) = q.purpose { query = query.bind(v); }
    if let Some(ref v) = q.tag { query = query.bind(v); }
    if let Some(v) = q.expiring_within_days { query = query.bind(v); }

    query = query.bind(per_page).bind(offset);

    let rows = query.fetch_all(&state.db).await?;

    Ok(Json(PaginatedResponse {
        data: rows,
        total,
        page,
        per_page,
    }))
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

    let row = sqlx::query_as::<_, Vps>(VPS_INSERT_SQL)
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
    .bind(input.provider_id.or(existing.provider_id))
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

// ─── Export ──────────────────────────────────────────────

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
struct ProviderNameRow {
    id: Uuid,
    name: String,
}

async fn export(
    State(state): State<AppState>,
    Query(q): Query<ExportQuery>,
) -> Result<impl IntoResponse, AppError> {
    let rows = sqlx::query_as::<_, Vps>(
        "SELECT * FROM vps ORDER BY status, expire_date ASC NULLS LAST, hostname",
    )
    .fetch_all(&state.db)
    .await?;

    let providers = sqlx::query_as::<_, ProviderNameRow>("SELECT id, name FROM providers")
        .fetch_all(&state.db)
        .await?;
    let provider_map: HashMap<Uuid, String> =
        providers.into_iter().map(|p| (p.id, p.name)).collect();

    match q.format.as_str() {
        "csv" => {
            let mut wtr = csv::Writer::from_writer(vec![]);
            for r in &rows {
                let provider_name = r.provider_id
                    .and_then(|pid| provider_map.get(&pid))
                    .cloned()
                    .unwrap_or_default();
                wtr.serialize(VpsCsvRow {
                    hostname: r.hostname.clone(),
                    alias: r.alias.clone(),
                    provider_name,
                    ip_addresses: serde_json::to_string(&r.ip_addresses.0).unwrap_or_default(),
                    ssh_port: r.ssh_port,
                    country: r.country.clone(),
                    city: r.city.clone(),
                    dc_name: r.dc_name.clone(),
                    cpu_cores: r.cpu_cores,
                    ram_mb: r.ram_mb,
                    disk_gb: r.disk_gb,
                    bandwidth_tb: r.bandwidth_tb.map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
                    cost_monthly: r.cost_monthly.map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
                    currency: r.currency.clone(),
                    status: r.status.clone(),
                    purchase_date: r.purchase_date.map(|d| d.to_string()).unwrap_or_default(),
                    expire_date: r.expire_date.map(|d| d.to_string()).unwrap_or_default(),
                    purpose: r.purpose.clone(),
                    vpn_protocol: r.vpn_protocol.clone(),
                    tags: r.tags.join(";"),
                    monitoring_enabled: r.monitoring_enabled,
                    node_exporter_port: r.node_exporter_port,
                    extra: serde_json::to_string(&r.extra).unwrap_or_default(),
                    notes: r.notes.clone(),
                })
                .map_err(|e| AppError::Internal(format!("CSV write error: {e}")))?;
            }
            let data = wtr
                .into_inner()
                .map_err(|e| AppError::Internal(format!("CSV flush error: {e}")))?;

            Ok((
                [
                    (header::CONTENT_TYPE, "text/csv".to_string()),
                    (
                        header::CONTENT_DISPOSITION,
                        "attachment; filename=\"vps.csv\"".to_string(),
                    ),
                ],
                data,
            )
                .into_response())
        }
        _ => {
            let json = serde_json::to_string_pretty(&rows)
                .map_err(|e| AppError::Internal(format!("JSON error: {e}")))?;

            Ok((
                [
                    (header::CONTENT_TYPE, "application/json".to_string()),
                    (
                        header::CONTENT_DISPOSITION,
                        "attachment; filename=\"vps.json\"".to_string(),
                    ),
                ],
                json,
            )
                .into_response())
        }
    }
}

// ─── Import ──────────────────────────────────────────────

async fn import(
    State(state): State<AppState>,
    Json(input): Json<ImportRequest>,
) -> Result<Json<ImportResult>, AppError> {
    // Build provider name → id lookup (case-insensitive)
    let providers = sqlx::query_as::<_, ProviderNameRow>("SELECT id, name FROM providers")
        .fetch_all(&state.db)
        .await?;
    let provider_lookup: HashMap<String, Uuid> = providers
        .into_iter()
        .map(|p| (p.name.to_lowercase(), p.id))
        .collect();

    let mut imported = 0usize;
    let mut errors = Vec::new();

    match input.format.as_str() {
        "csv" => {
            let mut rdr = csv::Reader::from_reader(input.data.as_bytes());
            for (i, result) in rdr.deserialize::<VpsCsvRow>().enumerate() {
                let row_num = i + 1;
                let row = match result {
                    Ok(r) => r,
                    Err(e) => {
                        errors.push(format!("Row {row_num}: parse error: {e}"));
                        continue;
                    }
                };

                if row.hostname.trim().is_empty() {
                    errors.push(format!("Row {row_num}: hostname is required"));
                    continue;
                }

                let provider_id = match provider_lookup.get(&row.provider_name.to_lowercase()) {
                    Some(id) => *id,
                    None => {
                        errors.push(format!(
                            "Row {row_num}: unknown provider '{}'",
                            row.provider_name
                        ));
                        continue;
                    }
                };

                match import_vps_csv_row(&state, &row, provider_id).await {
                    Ok(_) => imported += 1,
                    Err(e) => errors.push(format!("Row {row_num}: {e}")),
                }
            }
        }
        "json" => {
            let vps_list: Vec<CreateVps> = serde_json::from_str(&input.data)
                .map_err(|e| AppError::BadRequest(format!("Invalid JSON: {e}")))?;

            for (i, vps) in vps_list.iter().enumerate() {
                let row_num = i + 1;
                if vps.hostname.trim().is_empty() {
                    errors.push(format!("Row {row_num}: hostname is required"));
                    continue;
                }

                if let Err(e) = validate_ips(&vps.ip_addresses) {
                    errors.push(format!("Row {row_num}: {e}"));
                    continue;
                }

                let ip_json = match serde_json::to_value(&vps.ip_addresses) {
                    Ok(v) => v,
                    Err(e) => {
                        errors.push(format!("Row {row_num}: invalid ip_addresses: {e}"));
                        continue;
                    }
                };

                let result = sqlx::query(VPS_INSERT_SQL)
                    .bind(&vps.hostname)
                    .bind(&vps.alias)
                    .bind(vps.provider_id)
                    .bind(&ip_json)
                    .bind(vps.ssh_port)
                    .bind(&vps.country)
                    .bind(&vps.city)
                    .bind(&vps.dc_name)
                    .bind(vps.cpu_cores)
                    .bind(vps.ram_mb)
                    .bind(vps.disk_gb)
                    .bind(vps.bandwidth_tb.and_then(Decimal::from_f64_retain))
                    .bind(vps.cost_monthly.and_then(Decimal::from_f64_retain))
                    .bind(&vps.currency)
                    .bind(&vps.status)
                    .bind(vps.purchase_date)
                    .bind(vps.expire_date)
                    .bind(&vps.purpose)
                    .bind(&vps.vpn_protocol)
                    .bind(&vps.tags)
                    .bind(vps.monitoring_enabled)
                    .bind(vps.node_exporter_port)
                    .bind(&vps.extra)
                    .bind(&vps.notes)
                    .execute(&state.db)
                    .await;

                match result {
                    Ok(_) => imported += 1,
                    Err(e) => errors.push(format!("Row {row_num}: {e}")),
                }
            }
        }
        _ => return Err(AppError::BadRequest("format must be 'csv' or 'json'".into())),
    }

    Ok(Json(ImportResult { imported, errors }))
}

async fn import_vps_csv_row(
    state: &AppState,
    row: &VpsCsvRow,
    provider_id: Uuid,
) -> Result<(), AppError> {
    // Parse ip_addresses from JSON string
    let ip_entries: Vec<IpEntry> = if row.ip_addresses.trim().is_empty() {
        vec![]
    } else {
        serde_json::from_str(&row.ip_addresses)
            .map_err(|e| AppError::BadRequest(format!("invalid ip_addresses JSON: {e}")))?
    };
    validate_ips(&ip_entries)?;

    let ip_json = serde_json::to_value(&ip_entries)
        .map_err(|e| AppError::BadRequest(format!("ip_addresses serialize error: {e}")))?;

    // Parse tags from semicolon-separated
    let tags: Vec<String> = if row.tags.trim().is_empty() {
        vec![]
    } else {
        row.tags.split(';').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    };

    // Parse extra from JSON string
    let extra: serde_json::Value = if row.extra.trim().is_empty() {
        serde_json::Value::Object(serde_json::Map::new())
    } else {
        serde_json::from_str(&row.extra)
            .map_err(|e| AppError::BadRequest(format!("invalid extra JSON: {e}")))?
    };

    // Parse dates
    let purchase_date = if row.purchase_date.trim().is_empty() {
        None
    } else {
        Some(
            row.purchase_date
                .trim()
                .parse::<chrono::NaiveDate>()
                .map_err(|e| AppError::BadRequest(format!("invalid purchase_date: {e}")))?,
        )
    };

    let expire_date = if row.expire_date.trim().is_empty() {
        None
    } else {
        Some(
            row.expire_date
                .trim()
                .parse::<chrono::NaiveDate>()
                .map_err(|e| AppError::BadRequest(format!("invalid expire_date: {e}")))?,
        )
    };

    sqlx::query(VPS_INSERT_SQL)
        .bind(&row.hostname)
        .bind(&row.alias)
        .bind(provider_id)
        .bind(&ip_json)
        .bind(row.ssh_port)
        .bind(&row.country)
        .bind(&row.city)
        .bind(&row.dc_name)
        .bind(row.cpu_cores)
        .bind(row.ram_mb)
        .bind(row.disk_gb)
        .bind(row.bandwidth_tb.and_then(Decimal::from_f64_retain))
        .bind(row.cost_monthly.and_then(Decimal::from_f64_retain))
        .bind(&row.currency)
        .bind(&row.status)
        .bind(purchase_date)
        .bind(expire_date)
        .bind(&row.purpose)
        .bind(&row.vpn_protocol)
        .bind(&tags)
        .bind(row.monitoring_enabled)
        .bind(row.node_exporter_port)
        .bind(&extra)
        .bind(&row.notes)
        .execute(&state.db)
        .await?;

    Ok(())
}
