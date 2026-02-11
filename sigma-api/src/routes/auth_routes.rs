use axum::{extract::State, Extension, routing::post, Json, Router};

use crate::auth::{create_token, hash_password, require_role, verify_password, CurrentUser};
use crate::errors::{AppError, ErrorResponse};
use crate::models::{ChangePasswordRequest, LoginRequest, LoginResponse, User, UserResponse};
use crate::routes::AppState;

/// Public routes (no auth required)
pub fn router() -> Router<AppState> {
    Router::new().route("/api/auth/login", post(login))
}

/// Protected routes (auth required)
pub fn protected_router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/me", axum::routing::get(me))
        .route("/api/auth/refresh", post(refresh))
        .route("/api/auth/change-password", post(change_password))
}

#[utoipa::path(
    post, path = "/api/auth/login",
    tag = "Auth",
    request_body = LoginRequest,
    responses(
        (status = 200, body = LoginResponse),
        (status = 401, body = ErrorResponse),
    )
)]
async fn login(
    State(state): State<AppState>,
    Json(input): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(&input.email)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !verify_password(&input.password, &user.password_hash)? {
        return Err(AppError::Unauthorized);
    }

    let token = create_token(
        user.id,
        &user.email,
        &user.role,
        &state.jwt_secret,
        state.jwt_expiry_hours,
    )?;

    Ok(Json(LoginResponse {
        token,
        user: user.into(),
    }))
}

#[utoipa::path(
    get, path = "/api/auth/me",
    tag = "Auth",
    responses(
        (status = 200, body = UserResponse),
        (status = 401, body = ErrorResponse),
    ),
    security(("bearer" = []))
)]
async fn me(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
) -> Result<Json<UserResponse>, AppError> {
    if current.is_api_key {
        return Ok(Json(UserResponse {
            id: current.id,
            email: current.email.clone(),
            name: "API Key".to_string(),
            role: current.role.clone(),
            force_password_change: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }));
    }

    let db_user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(current.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;

    Ok(Json(db_user.into()))
}

#[utoipa::path(
    post, path = "/api/auth/refresh",
    tag = "Auth",
    responses(
        (status = 200, body = LoginResponse),
        (status = 401, body = ErrorResponse),
    ),
    security(("bearer" = []))
)]
async fn refresh(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
) -> Result<Json<LoginResponse>, AppError> {
    require_role(&current, &["admin", "operator", "readonly"])?;

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(current.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;

    let token = create_token(
        user.id,
        &user.email,
        &user.role,
        &state.jwt_secret,
        state.jwt_expiry_hours,
    )?;

    Ok(Json(LoginResponse {
        token,
        user: user.into(),
    }))
}

#[utoipa::path(
    post, path = "/api/auth/change-password",
    tag = "Auth",
    request_body = ChangePasswordRequest,
    responses(
        (status = 200, body = UserResponse),
        (status = 400, body = ErrorResponse),
        (status = 401, body = ErrorResponse),
    ),
    security(("bearer" = []))
)]
async fn change_password(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Json(input): Json<ChangePasswordRequest>,
) -> Result<Json<UserResponse>, AppError> {
    if current.is_api_key {
        return Err(AppError::BadRequest("Cannot change password with API key auth".into()));
    }

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(current.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !verify_password(&input.current_password, &user.password_hash)? {
        return Err(AppError::BadRequest("Current password is incorrect".into()));
    }

    if input.new_password.len() < 6 {
        return Err(AppError::BadRequest("New password must be at least 6 characters".into()));
    }

    let new_hash = hash_password(&input.new_password)?;

    let updated = sqlx::query_as::<_, User>(
        "UPDATE users SET password_hash = $1, force_password_change = false WHERE id = $2 RETURNING *",
    )
    .bind(&new_hash)
    .bind(user.id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(updated.into()))
}
