use axum::{
    extract::{Path, Query, State},
    routing::get,
    Extension, Json, Router,
};
use uuid::Uuid;

use crate::auth::{require_role, CurrentUser};
use crate::errors::AppError;
use crate::models::{
    CreateTicket, CreateTicketComment, PaginatedResponse, PaginatedTicketResponse, Ticket,
    TicketComment, TicketListQuery, UpdateTicket,
};
use crate::routes::audit_logs::log_audit;
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/tickets", get(list).post(create))
        .route("/api/tickets/{id}", get(get_one).put(update).delete(delete))
        .route(
            "/api/tickets/{id}/comments",
            get(list_comments).post(add_comment),
        )
}

#[utoipa::path(
    get,
    path = "/api/tickets",
    tag = "Tickets",
    params(TicketListQuery),
    responses(
        (status = 200, body = PaginatedTicketResponse),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    Query(q): Query<TicketListQuery>,
) -> Result<Json<PaginatedResponse<Ticket>>, AppError> {
    let per_page = q.per_page.clamp(1, 100);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let mut where_clause = String::from(" WHERE 1=1");
    let mut param_idx = 0u32;

    if q.status.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND status = ${param_idx}"));
    }
    if q.priority.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND priority = ${param_idx}"));
    }
    if q.assigned_to.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND assigned_to = ${param_idx}"));
    }
    if q.vps_id.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND vps_id = ${param_idx}"));
    }

    // Count
    let count_sql = format!("SELECT COUNT(*) FROM tickets{where_clause}");
    let mut count_query = sqlx::query_as::<_, (i64,)>(&count_sql);
    if let Some(ref v) = q.status {
        count_query = count_query.bind(v);
    }
    if let Some(ref v) = q.priority {
        count_query = count_query.bind(v);
    }
    if let Some(ref v) = q.assigned_to {
        count_query = count_query.bind(v);
    }
    if let Some(ref v) = q.vps_id {
        count_query = count_query.bind(v);
    }
    let total = count_query.fetch_one(&state.db).await?.0;

    // Data
    param_idx += 1;
    let limit_param = param_idx;
    param_idx += 1;
    let offset_param = param_idx;

    let data_sql = format!(
        "SELECT * FROM tickets{where_clause} ORDER BY created_at DESC LIMIT ${limit_param} OFFSET ${offset_param}"
    );
    let mut query = sqlx::query_as::<_, Ticket>(&data_sql);
    if let Some(ref v) = q.status {
        query = query.bind(v);
    }
    if let Some(ref v) = q.priority {
        query = query.bind(v);
    }
    if let Some(ref v) = q.assigned_to {
        query = query.bind(v);
    }
    if let Some(ref v) = q.vps_id {
        query = query.bind(v);
    }
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
    post,
    path = "/api/tickets",
    tag = "Tickets",
    request_body = CreateTicket,
    responses(
        (status = 200, body = Ticket),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Json(input): Json<CreateTicket>,
) -> Result<Json<Ticket>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    let row = sqlx::query_as::<_, Ticket>(
        r#"INSERT INTO tickets (title, description, priority, vps_id, provider_id, created_by, assigned_to)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING *"#,
    )
    .bind(&input.title)
    .bind(&input.description)
    .bind(&input.priority)
    .bind(input.vps_id)
    .bind(input.provider_id)
    .bind(user.id)
    .bind(input.assigned_to)
    .fetch_one(&state.db)
    .await?;

    log_audit(
        &state.db,
        &user,
        "create",
        "ticket",
        Some(&row.id.to_string()),
        serde_json::json!({"title": row.title}),
    )
    .await;

    Ok(Json(row))
}

#[utoipa::path(
    get,
    path = "/api/tickets/{id}",
    tag = "Tickets",
    params(("id" = Uuid, Path, description = "Ticket ID")),
    responses(
        (status = 200, body = Ticket),
        (status = 404),
    )
)]
pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Ticket>, AppError> {
    let row = sqlx::query_as::<_, Ticket>("SELECT * FROM tickets WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(row))
}

#[utoipa::path(
    put,
    path = "/api/tickets/{id}",
    tag = "Tickets",
    params(("id" = Uuid, Path, description = "Ticket ID")),
    request_body = UpdateTicket,
    responses(
        (status = 200, body = Ticket),
        (status = 404),
    )
)]
pub async fn update(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateTicket>,
) -> Result<Json<Ticket>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    let existing = sqlx::query_as::<_, Ticket>("SELECT * FROM tickets WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let row = sqlx::query_as::<_, Ticket>(
        r#"UPDATE tickets SET
            title = $2, description = $3, status = $4, priority = $5,
            vps_id = $6, provider_id = $7, assigned_to = $8, updated_at = now()
           WHERE id = $1
           RETURNING *"#,
    )
    .bind(id)
    .bind(input.title.unwrap_or(existing.title))
    .bind(input.description.unwrap_or(existing.description))
    .bind(input.status.unwrap_or(existing.status))
    .bind(input.priority.unwrap_or(existing.priority))
    .bind(input.vps_id.unwrap_or(existing.vps_id))
    .bind(input.provider_id.unwrap_or(existing.provider_id))
    .bind(input.assigned_to.unwrap_or(existing.assigned_to))
    .fetch_one(&state.db)
    .await?;

    log_audit(
        &state.db,
        &user,
        "update",
        "ticket",
        Some(&id.to_string()),
        serde_json::json!({"title": row.title, "status": row.status}),
    )
    .await;

    Ok(Json(row))
}

#[utoipa::path(
    delete,
    path = "/api/tickets/{id}",
    tag = "Tickets",
    params(("id" = Uuid, Path, description = "Ticket ID")),
    responses(
        (status = 200),
        (status = 404),
    )
)]
pub async fn delete(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_role(&user, &["admin"])?;

    let result = sqlx::query("DELETE FROM tickets WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    log_audit(
        &state.db,
        &user,
        "delete",
        "ticket",
        Some(&id.to_string()),
        serde_json::json!({}),
    )
    .await;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

#[utoipa::path(
    get,
    path = "/api/tickets/{id}/comments",
    tag = "Tickets",
    params(("id" = Uuid, Path, description = "Ticket ID")),
    responses(
        (status = 200, body = Vec<TicketComment>),
    )
)]
pub async fn list_comments(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<TicketComment>>, AppError> {
    let rows = sqlx::query_as::<_, TicketComment>(
        "SELECT * FROM ticket_comments WHERE ticket_id = $1 ORDER BY created_at ASC",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows))
}

#[utoipa::path(
    post,
    path = "/api/tickets/{id}/comments",
    tag = "Tickets",
    params(("id" = Uuid, Path, description = "Ticket ID")),
    request_body = CreateTicketComment,
    responses(
        (status = 200, body = TicketComment),
    )
)]
pub async fn add_comment(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
    Json(input): Json<CreateTicketComment>,
) -> Result<Json<TicketComment>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    // Verify ticket exists
    sqlx::query_as::<_, Ticket>("SELECT * FROM tickets WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let row = sqlx::query_as::<_, TicketComment>(
        r#"INSERT INTO ticket_comments (ticket_id, user_id, user_email, body)
           VALUES ($1, $2, $3, $4)
           RETURNING *"#,
    )
    .bind(id)
    .bind(user.id)
    .bind(&user.email)
    .bind(&input.body)
    .fetch_one(&state.db)
    .await?;

    log_audit(
        &state.db,
        &user,
        "comment",
        "ticket",
        Some(&id.to_string()),
        serde_json::json!({"comment_id": row.id.to_string()}),
    )
    .await;

    Ok(Json(row))
}
