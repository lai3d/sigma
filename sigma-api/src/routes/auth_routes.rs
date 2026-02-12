use axum::{extract::State, Extension, routing::post, Json, Router};

use crate::auth::{
    create_token, create_totp_challenge_token, generate_totp_secret, hash_password, require_role,
    verify_password, verify_totp_challenge_token, verify_totp_code, CurrentUser,
};
use crate::errors::{AppError, ErrorResponse};
use crate::models::{
    ChangePasswordRequest, LoginRequest, LoginResponse, TotpChallengeResponse, TotpDisableRequest,
    TotpLoginRequest, TotpSetupResponse, TotpVerifyRequest, User, UserResponse,
};
use crate::routes::audit_logs::log_audit;
use crate::routes::AppState;

/// Public routes (no auth required)
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/login", post(login))
        .route("/api/auth/login/totp", post(login_totp))
}

/// Protected routes (auth required)
pub fn protected_router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/me", axum::routing::get(me))
        .route("/api/auth/refresh", post(refresh))
        .route("/api/auth/change-password", post(change_password))
        .route("/api/auth/totp/setup", post(totp_setup))
        .route("/api/auth/totp/verify", post(totp_verify))
        .route("/api/auth/totp/disable", post(totp_disable))
}

#[utoipa::path(
    post, path = "/api/auth/login",
    tag = "Auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login success or TOTP challenge", body = Value),
        (status = 401, body = ErrorResponse),
    )
)]
pub async fn login(
    State(state): State<AppState>,
    Json(input): Json<LoginRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(&input.email)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !verify_password(&input.password, &user.password_hash)? {
        return Err(AppError::Unauthorized);
    }

    if user.totp_enabled {
        let totp_token = create_totp_challenge_token(user.id, &state.jwt_secret)?;
        let challenge = TotpChallengeResponse {
            requires_totp: true,
            totp_token,
        };
        return Ok(Json(serde_json::to_value(challenge).unwrap()));
    }

    let token = create_token(
        user.id,
        &user.email,
        &user.role,
        &state.jwt_secret,
        state.jwt_expiry_hours,
    )?;

    let login_user = CurrentUser { id: user.id, email: user.email.clone(), role: user.role.clone(), is_api_key: false };
    log_audit(&state.db, &login_user, "login", "user", Some(&user.id.to_string()),
        serde_json::json!({"email": user.email})).await;

    let resp = LoginResponse {
        token,
        user: user.into(),
    };
    Ok(Json(serde_json::to_value(resp).unwrap()))
}

#[utoipa::path(
    post, path = "/api/auth/login/totp",
    tag = "Auth",
    request_body = TotpLoginRequest,
    responses(
        (status = 200, body = LoginResponse),
        (status = 401, body = ErrorResponse),
    )
)]
pub async fn login_totp(
    State(state): State<AppState>,
    Json(input): Json<TotpLoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let user_id = verify_totp_challenge_token(&input.totp_token, &state.jwt_secret)?;

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;

    let secret = user.totp_secret.as_deref().ok_or(AppError::Unauthorized)?;
    if !verify_totp_code(secret, &user.email, &input.code)? {
        return Err(AppError::BadRequest("Invalid TOTP code".into()));
    }

    let token = create_token(
        user.id,
        &user.email,
        &user.role,
        &state.jwt_secret,
        state.jwt_expiry_hours,
    )?;

    let login_user = CurrentUser { id: user.id, email: user.email.clone(), role: user.role.clone(), is_api_key: false };
    log_audit(&state.db, &login_user, "login", "user", Some(&user.id.to_string()),
        serde_json::json!({"email": user.email, "totp": true})).await;

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
pub async fn me(
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
            totp_enabled: false,
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

// ─── TOTP setup / verify / disable ──────────────────────

#[utoipa::path(
    post, path = "/api/auth/totp/setup",
    tag = "Auth",
    responses(
        (status = 200, body = TotpSetupResponse),
        (status = 400, body = ErrorResponse),
        (status = 401, body = ErrorResponse),
    ),
    security(("bearer" = []))
)]
pub async fn totp_setup(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
) -> Result<Json<TotpSetupResponse>, AppError> {
    if current.is_api_key {
        return Err(AppError::BadRequest("Cannot setup TOTP with API key auth".into()));
    }

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(current.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if user.totp_enabled {
        return Err(AppError::BadRequest("TOTP is already enabled. Disable it first.".into()));
    }

    let (secret_base32, totp) = generate_totp_secret(&user.email)?;
    let otpauth_url = totp.get_url();
    let qr_code = totp
        .get_qr_base64()
        .map_err(|e| AppError::Internal(format!("QR code generation failed: {e}")))?;

    // Store secret (not yet enabled)
    sqlx::query("UPDATE users SET totp_secret = $1 WHERE id = $2")
        .bind(&secret_base32)
        .bind(user.id)
        .execute(&state.db)
        .await?;

    Ok(Json(TotpSetupResponse {
        secret: secret_base32,
        otpauth_url,
        qr_code,
    }))
}

#[utoipa::path(
    post, path = "/api/auth/totp/verify",
    tag = "Auth",
    request_body = TotpVerifyRequest,
    responses(
        (status = 200, description = "TOTP enabled"),
        (status = 400, body = ErrorResponse),
        (status = 401, body = ErrorResponse),
    ),
    security(("bearer" = []))
)]
pub async fn totp_verify(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Json(input): Json<TotpVerifyRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(current.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;

    let secret = user
        .totp_secret
        .as_deref()
        .ok_or(AppError::BadRequest("TOTP not set up. Call /api/auth/totp/setup first.".into()))?;

    if !verify_totp_code(secret, &user.email, &input.code)? {
        return Err(AppError::BadRequest("Invalid TOTP code".into()));
    }

    sqlx::query("UPDATE users SET totp_enabled = true WHERE id = $1")
        .bind(user.id)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({ "totp_enabled": true })))
}

#[utoipa::path(
    post, path = "/api/auth/totp/disable",
    tag = "Auth",
    request_body = TotpDisableRequest,
    responses(
        (status = 200, description = "TOTP disabled"),
        (status = 400, body = ErrorResponse),
        (status = 401, body = ErrorResponse),
    ),
    security(("bearer" = []))
)]
pub async fn totp_disable(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Json(input): Json<TotpDisableRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(current.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !user.totp_enabled {
        return Err(AppError::BadRequest("TOTP is not enabled".into()));
    }

    let secret = user.totp_secret.as_deref().ok_or(AppError::Unauthorized)?;
    if !verify_totp_code(secret, &user.email, &input.code)? {
        return Err(AppError::BadRequest("Invalid TOTP code".into()));
    }

    sqlx::query("UPDATE users SET totp_secret = NULL, totp_enabled = false WHERE id = $1")
        .bind(user.id)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({ "totp_enabled": false })))
}
