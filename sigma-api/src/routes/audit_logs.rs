use axum::{
    extract::{Query, State},
    routing::get,
    Extension, Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{require_role, CurrentUser};
use crate::db::Db;
use crate::errors::AppError;
use crate::models::PaginatedResponse;
use crate::routes::AppState;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AuditLog {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub user_email: String,
    pub action: String,
    pub resource: String,
    pub resource_id: Option<String>,
    pub details: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct AuditLogQuery {
    pub resource: Option<String>,
    pub resource_id: Option<String>,
    pub user_id: Option<Uuid>,
    pub action: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

fn default_page() -> i64 { 1 }
fn default_per_page() -> i64 { 50 }

pub fn router() -> Router<AppState> {
    Router::new().route("/api/audit-logs", get(list))
}

/// Insert an audit log entry. Best-effort: failures are logged, not propagated.
pub async fn log_audit(
    db: &Db,
    user: &CurrentUser,
    action: &str,
    resource: &str,
    resource_id: Option<&str>,
    details: serde_json::Value,
) {
    let user_id = if user.id.is_nil() { None } else { Some(user.id) };
    let result = sqlx::query(
        "INSERT INTO audit_logs (user_id, user_email, action, resource, resource_id, details) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(user_id)
    .bind(&user.email)
    .bind(action)
    .bind(resource)
    .bind(resource_id)
    .bind(&details)
    .execute(db)
    .await;

    if let Err(e) = result {
        tracing::warn!("Failed to write audit log: {e}");
    }
}

async fn list(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Query(q): Query<AuditLogQuery>,
) -> Result<Json<PaginatedResponse<AuditLog>>, AppError> {
    require_role(&user, &["admin"])?;

    let per_page = q.per_page.clamp(1, 100);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let mut where_clause = String::from(" WHERE 1=1");
    let mut param_idx = 0u32;

    if q.resource.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND resource = ${param_idx}"));
    }
    if q.resource_id.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND resource_id = ${param_idx}"));
    }
    if q.user_id.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND user_id = ${param_idx}"));
    }
    if q.action.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND action = ${param_idx}"));
    }
    if q.since.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND created_at >= ${param_idx}::timestamptz"));
    }
    if q.until.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND created_at <= ${param_idx}::timestamptz"));
    }

    // Count query
    let count_sql = format!("SELECT COUNT(*) FROM audit_logs{where_clause}");
    let mut count_query = sqlx::query_as::<_, (i64,)>(&count_sql);
    if let Some(ref v) = q.resource { count_query = count_query.bind(v); }
    if let Some(ref v) = q.resource_id { count_query = count_query.bind(v); }
    if let Some(ref v) = q.user_id { count_query = count_query.bind(v); }
    if let Some(ref v) = q.action { count_query = count_query.bind(v); }
    if let Some(ref v) = q.since { count_query = count_query.bind(v); }
    if let Some(ref v) = q.until { count_query = count_query.bind(v); }

    let total = count_query.fetch_one(&state.db).await?.0;

    // Data query
    param_idx += 1;
    let limit_param = param_idx;
    param_idx += 1;
    let offset_param = param_idx;

    let data_sql = format!(
        "SELECT * FROM audit_logs{where_clause} ORDER BY created_at DESC LIMIT ${limit_param} OFFSET ${offset_param}"
    );
    let mut query = sqlx::query_as::<_, AuditLog>(&data_sql);
    if let Some(ref v) = q.resource { query = query.bind(v); }
    if let Some(ref v) = q.resource_id { query = query.bind(v); }
    if let Some(ref v) = q.user_id { query = query.bind(v); }
    if let Some(ref v) = q.action { query = query.bind(v); }
    if let Some(ref v) = q.since { query = query.bind(v); }
    if let Some(ref v) = q.until { query = query.bind(v); }
    query = query.bind(per_page).bind(offset);

    let rows = query.fetch_all(&state.db).await?;

    Ok(Json(PaginatedResponse {
        data: rows,
        total,
        page,
        per_page,
    }))
}
