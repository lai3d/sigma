use axum::{
    extract::{Path, Query, State},
    routing::get,
    Extension, Json, Router,
};
use uuid::Uuid;

use crate::auth::{hash_password, require_role, CurrentUser};
use crate::errors::{AppError, ErrorResponse};
#[allow(unused_imports)]
use crate::models::PaginatedUserResponse;
use crate::models::{CreateUser, PaginatedResponse, UpdateUser, User, UserListQuery, UserResponse};
use crate::routes::audit_logs::log_audit;
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/users", get(list).post(create))
        .route("/api/users/{id}", get(get_one).put(update).delete(delete))
}

fn validate_role(role: &str) -> Result<(), AppError> {
    match role {
        "admin" | "operator" | "readonly" => Ok(()),
        _ => Err(AppError::BadRequest(
            "role must be one of: admin, operator, readonly".into(),
        )),
    }
}

#[utoipa::path(
    get, path = "/api/users",
    tag = "Users",
    params(UserListQuery),
    responses(
        (status = 200, body = PaginatedUserResponse),
        (status = 403, body = ErrorResponse),
    ),
    security(("bearer" = []))
)]
pub async fn list(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Query(q): Query<UserListQuery>,
) -> Result<Json<PaginatedResponse<UserResponse>>, AppError> {
    require_role(&current, &["admin"])?;

    let per_page = q.per_page.clamp(1, 100);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let (total, rows) = if let Some(ref role) = q.role {
        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE role = $1")
            .bind(role)
            .fetch_one(&state.db)
            .await?;
        let rows = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE role = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(role)
        .bind(per_page)
        .bind(offset)
        .fetch_all(&state.db)
        .await?;
        (total.0, rows)
    } else {
        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&state.db)
            .await?;
        let rows = sqlx::query_as::<_, User>(
            "SELECT * FROM users ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(per_page)
        .bind(offset)
        .fetch_all(&state.db)
        .await?;
        (total.0, rows)
    };

    Ok(Json(PaginatedResponse {
        data: rows.into_iter().map(UserResponse::from).collect(),
        total,
        page,
        per_page,
    }))
}

#[utoipa::path(
    get, path = "/api/users/{id}",
    tag = "Users",
    params(("id" = Uuid, Path, description = "User ID")),
    responses(
        (status = 200, body = UserResponse),
        (status = 404, body = ErrorResponse),
    ),
    security(("bearer" = []))
)]
pub async fn get_one(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<UserResponse>, AppError> {
    require_role(&current, &["admin"])?;

    let row = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(row.into()))
}

#[utoipa::path(
    post, path = "/api/users",
    tag = "Users",
    request_body = CreateUser,
    responses(
        (status = 200, body = UserResponse),
        (status = 400, body = ErrorResponse),
        (status = 403, body = ErrorResponse),
    ),
    security(("bearer" = []))
)]
pub async fn create(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Json(input): Json<CreateUser>,
) -> Result<Json<UserResponse>, AppError> {
    require_role(&current, &["admin"])?;

    validate_role(&input.role)?;

    if input.password.len() < 6 {
        return Err(AppError::BadRequest("Password must be at least 6 characters".into()));
    }

    let password_hash = hash_password(&input.password)?;

    let row = sqlx::query_as::<_, User>(
        "INSERT INTO users (email, password_hash, name, role) VALUES ($1, $2, $3, $4) RETURNING *",
    )
    .bind(&input.email)
    .bind(&password_hash)
    .bind(&input.name)
    .bind(&input.role)
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db_err) if db_err.constraint() == Some("users_email_key") => {
            AppError::BadRequest(format!("Email '{}' already exists", input.email))
        }
        _ => AppError::from(e),
    })?;

    log_audit(&state.db, &current, "create", "user", Some(&row.id.to_string()),
        serde_json::json!({"email": row.email, "role": row.role})).await;

    Ok(Json(row.into()))
}

#[utoipa::path(
    put, path = "/api/users/{id}",
    tag = "Users",
    params(("id" = Uuid, Path, description = "User ID")),
    request_body = UpdateUser,
    responses(
        (status = 200, body = UserResponse),
        (status = 404, body = ErrorResponse),
        (status = 403, body = ErrorResponse),
    ),
    security(("bearer" = []))
)]
pub async fn update(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateUser>,
) -> Result<Json<UserResponse>, AppError> {
    require_role(&current, &["admin"])?;

    let existing = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let role = match input.role {
        Some(ref r) => {
            validate_role(r)?;
            r.clone()
        }
        None => existing.role,
    };

    let password_hash = match input.password {
        Some(ref p) => {
            if p.len() < 6 {
                return Err(AppError::BadRequest("Password must be at least 6 characters".into()));
            }
            hash_password(p)?
        }
        None => existing.password_hash,
    };

    // Admin force-disable TOTP: if totp_enabled is explicitly set to false, clear secret
    let (totp_enabled, totp_secret) = if input.totp_enabled == Some(false) {
        (false, None)
    } else {
        (existing.totp_enabled, existing.totp_secret)
    };

    let row = sqlx::query_as::<_, User>(
        r#"UPDATE users SET
            email = $2, name = $3, role = $4, password_hash = $5, force_password_change = $6,
            totp_enabled = $7, totp_secret = $8
           WHERE id = $1
           RETURNING *"#,
    )
    .bind(id)
    .bind(input.email.unwrap_or(existing.email))
    .bind(input.name.unwrap_or(existing.name))
    .bind(&role)
    .bind(&password_hash)
    .bind(input.force_password_change.unwrap_or(existing.force_password_change))
    .bind(totp_enabled)
    .bind(totp_secret)
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db_err) if db_err.constraint() == Some("users_email_key") => {
            AppError::BadRequest("Email already exists".into())
        }
        _ => AppError::from(e),
    })?;

    log_audit(&state.db, &current, "update", "user", Some(&id.to_string()),
        serde_json::json!({"email": row.email, "role": row.role})).await;

    Ok(Json(row.into()))
}

#[utoipa::path(
    delete, path = "/api/users/{id}",
    tag = "Users",
    params(("id" = Uuid, Path, description = "User ID")),
    responses(
        (status = 200, description = "User deleted"),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
    ),
    security(("bearer" = []))
)]
pub async fn delete(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_role(&current, &["admin"])?;

    if current.id == id {
        return Err(AppError::BadRequest("Cannot delete yourself".into()));
    }

    let result = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    log_audit(&state.db, &current, "delete", "user", Some(&id.to_string()),
        serde_json::json!({})).await;

    Ok(Json(serde_json::json!({ "deleted": true })))
}
