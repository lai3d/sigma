use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::errors::{AppError, ErrorResponse};
#[allow(unused_imports)]
use crate::models::PaginatedIpCheckResponse;
use crate::models::{
    CreateIpCheck, IpCheck, IpCheckListQuery, IpCheckSummary, IpCheckSummaryQuery,
    PaginatedResponse, PurgeQuery,
};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/ip-checks", get(list).post(create))
        .route("/api/ip-checks/summary", get(summary))
        .route("/api/ip-checks/purge", axum::routing::delete(purge))
        .route("/api/ip-checks/{id}", get(get_one).delete(delete))
}

#[utoipa::path(
    get, path = "/api/ip-checks",
    tag = "IP Checks",
    params(IpCheckListQuery),
    responses(
        (status = 200, body = PaginatedIpCheckResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    Query(q): Query<IpCheckListQuery>,
) -> Result<Json<PaginatedResponse<IpCheck>>, AppError> {
    let per_page = q.per_page.clamp(1, 100);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let mut where_clause = String::from(" WHERE 1=1");
    let mut param_idx = 0u32;

    if q.vps_id.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND vps_id = ${}", param_idx));
    }
    if q.ip.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND ip = ${}::INET", param_idx));
    }
    if q.source.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND source = ${}", param_idx));
    }
    if q.check_type.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND check_type = ${}", param_idx));
    }
    if q.success.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND success = ${}", param_idx));
    }

    // Count query
    let count_sql = format!("SELECT COUNT(*) FROM ip_checks{}", where_clause);
    let mut count_query = sqlx::query_as::<_, (i64,)>(&count_sql);

    if let Some(ref v) = q.vps_id { count_query = count_query.bind(v); }
    if let Some(ref v) = q.ip { count_query = count_query.bind(v); }
    if let Some(ref v) = q.source { count_query = count_query.bind(v); }
    if let Some(ref v) = q.check_type { count_query = count_query.bind(v); }
    if let Some(v) = q.success { count_query = count_query.bind(v); }

    let total = count_query.fetch_one(&state.db).await?.0;

    // Data query
    param_idx += 1;
    let limit_param = param_idx;
    param_idx += 1;
    let offset_param = param_idx;

    let data_sql = format!(
        "SELECT id, vps_id, host(ip) as ip, check_type, source, success, latency_ms, checked_at \
         FROM ip_checks{} ORDER BY checked_at DESC LIMIT ${} OFFSET ${}",
        where_clause, limit_param, offset_param
    );
    let mut query = sqlx::query_as::<_, IpCheck>(&data_sql);

    if let Some(ref v) = q.vps_id { query = query.bind(v); }
    if let Some(ref v) = q.ip { query = query.bind(v); }
    if let Some(ref v) = q.source { query = query.bind(v); }
    if let Some(ref v) = q.check_type { query = query.bind(v); }
    if let Some(v) = q.success { query = query.bind(v); }

    query = query.bind(per_page).bind(offset);

    let rows = query.fetch_all(&state.db).await?;

    Ok(Json(PaginatedResponse {
        data: rows,
        total,
        page,
        per_page,
    }))
}

#[utoipa::path(
    get, path = "/api/ip-checks/{id}",
    tag = "IP Checks",
    params(("id" = Uuid, Path, description = "IP check ID")),
    responses(
        (status = 200, body = IpCheck),
        (status = 404, body = ErrorResponse),
    )
)]
pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<IpCheck>, AppError> {
    let row = sqlx::query_as::<_, IpCheck>(
        "SELECT id, vps_id, host(ip) as ip, check_type, source, success, latency_ms, checked_at \
         FROM ip_checks WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

#[utoipa::path(
    post, path = "/api/ip-checks",
    tag = "IP Checks",
    request_body = CreateIpCheck,
    responses(
        (status = 200, body = IpCheck),
        (status = 400, body = ErrorResponse),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateIpCheck>,
) -> Result<Json<IpCheck>, AppError> {
    // Validate check_type
    match input.check_type.as_str() {
        "icmp" | "tcp" | "http" => {}
        _ => {
            return Err(AppError::BadRequest(
                "check_type must be one of: icmp, tcp, http".into(),
            ))
        }
    }

    // Validate IP
    input
        .ip
        .parse::<std::net::IpAddr>()
        .map_err(|_| AppError::BadRequest(format!("Invalid IP address: '{}'", input.ip)))?;

    // Verify vps_id exists
    let vps_exists =
        sqlx::query_as::<_, (bool,)>("SELECT EXISTS(SELECT 1 FROM vps WHERE id = $1)")
            .bind(input.vps_id)
            .fetch_one(&state.db)
            .await?
            .0;

    if !vps_exists {
        return Err(AppError::BadRequest(format!(
            "VPS not found: {}",
            input.vps_id
        )));
    }

    let row = sqlx::query_as::<_, IpCheck>(
        "INSERT INTO ip_checks (vps_id, ip, check_type, source, success, latency_ms) \
         VALUES ($1, $2::INET, $3, $4, $5, $6) \
         RETURNING id, vps_id, host(ip) as ip, check_type, source, success, latency_ms, checked_at",
    )
    .bind(input.vps_id)
    .bind(&input.ip)
    .bind(&input.check_type)
    .bind(&input.source)
    .bind(input.success)
    .bind(input.latency_ms)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(row))
}

#[utoipa::path(
    delete, path = "/api/ip-checks/{id}",
    tag = "IP Checks",
    params(("id" = Uuid, Path, description = "IP check ID")),
    responses(
        (status = 200, description = "IP check deleted"),
        (status = 404, body = ErrorResponse),
    )
)]
pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let result = sqlx::query("DELETE FROM ip_checks WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(serde_json::json!({ "deleted": true })))
}

#[utoipa::path(
    get, path = "/api/ip-checks/summary",
    tag = "IP Checks",
    params(IpCheckSummaryQuery),
    responses(
        (status = 200, body = Vec<IpCheckSummary>),
    )
)]
pub async fn summary(
    State(state): State<AppState>,
    Query(q): Query<IpCheckSummaryQuery>,
) -> Result<Json<Vec<IpCheckSummary>>, AppError> {
    let (sql, bind_vps) = if q.vps_id.is_some() {
        (
            "SELECT \
                vps_id, \
                host(ip) as ip, \
                COUNT(*) as total_checks, \
                COUNT(*) FILTER (WHERE success) as success_count, \
                ROUND(COUNT(*) FILTER (WHERE success)::numeric / COUNT(*)::numeric * 100, 2)::float8 as success_rate, \
                AVG(latency_ms)::float8 as avg_latency_ms, \
                MAX(checked_at) as last_check, \
                (ARRAY_AGG(success ORDER BY checked_at DESC))[1] as last_success \
             FROM ip_checks \
             WHERE vps_id = $1 \
             GROUP BY vps_id, ip \
             ORDER BY last_check DESC",
            true,
        )
    } else {
        (
            "SELECT \
                vps_id, \
                host(ip) as ip, \
                COUNT(*) as total_checks, \
                COUNT(*) FILTER (WHERE success) as success_count, \
                ROUND(COUNT(*) FILTER (WHERE success)::numeric / COUNT(*)::numeric * 100, 2)::float8 as success_rate, \
                AVG(latency_ms)::float8 as avg_latency_ms, \
                MAX(checked_at) as last_check, \
                (ARRAY_AGG(success ORDER BY checked_at DESC))[1] as last_success \
             FROM ip_checks \
             GROUP BY vps_id, ip \
             ORDER BY last_check DESC",
            false,
        )
    };

    let rows = if bind_vps {
        sqlx::query_as::<_, IpCheckSummary>(sql)
            .bind(q.vps_id.unwrap())
            .fetch_all(&state.db)
            .await?
    } else {
        sqlx::query_as::<_, IpCheckSummary>(sql)
            .fetch_all(&state.db)
            .await?
    };

    Ok(Json(rows))
}

#[utoipa::path(
    delete, path = "/api/ip-checks/purge",
    tag = "IP Checks",
    params(PurgeQuery),
    responses(
        (status = 200, description = "Number of deleted records"),
    )
)]
pub async fn purge(
    State(state): State<AppState>,
    Query(q): Query<PurgeQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let result = sqlx::query(
        "DELETE FROM ip_checks WHERE checked_at < NOW() - ($1 || ' days')::INTERVAL",
    )
    .bind(q.older_than_days)
    .execute(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "deleted": result.rows_affected()
    })))
}
