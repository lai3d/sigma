use axum::{
    extract::{Path, Query, State},
    routing::get,
    Extension, Json, Router,
};
use uuid::Uuid;

use crate::auth::{require_role, CurrentUser};
use crate::errors::{AppError, ErrorResponse};
#[allow(unused_imports)]
use crate::models::PaginatedVpsPurposeResponse;
use crate::models::{
    CreateVpsPurpose, PaginatedResponse, UpdateVpsPurpose, VpsPurpose, VpsPurposeListQuery,
};
use crate::routes::audit_logs::log_audit;
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/vps-purposes", get(list).post(create))
        .route(
            "/api/vps-purposes/{id}",
            get(get_one).put(update).delete(delete),
        )
}

#[utoipa::path(
    get, path = "/api/vps-purposes",
    tag = "VPS Purposes",
    params(VpsPurposeListQuery),
    responses(
        (status = 200, body = PaginatedVpsPurposeResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    Query(q): Query<VpsPurposeListQuery>,
) -> Result<Json<PaginatedResponse<VpsPurpose>>, AppError> {
    let per_page = q.per_page.clamp(1, 100);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM vps_purposes")
        .fetch_one(&state.db)
        .await?;

    let rows = sqlx::query_as::<_, VpsPurpose>(
        "SELECT * FROM vps_purposes ORDER BY sort_order, name LIMIT $1 OFFSET $2",
    )
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(PaginatedResponse {
        data: rows,
        total: total.0,
        page,
        per_page,
    }))
}

#[utoipa::path(
    get, path = "/api/vps-purposes/{id}",
    tag = "VPS Purposes",
    params(("id" = Uuid, Path, description = "VPS Purpose ID")),
    responses(
        (status = 200, body = VpsPurpose),
        (status = 404, body = ErrorResponse),
    )
)]
pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<VpsPurpose>, AppError> {
    let row = sqlx::query_as::<_, VpsPurpose>("SELECT * FROM vps_purposes WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

#[utoipa::path(
    post, path = "/api/vps-purposes",
    tag = "VPS Purposes",
    request_body = CreateVpsPurpose,
    responses(
        (status = 200, body = VpsPurpose),
        (status = 400, body = ErrorResponse),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Json(input): Json<CreateVpsPurpose>,
) -> Result<Json<VpsPurpose>, AppError> {
    require_role(&user, &["admin", "operator"])?;
    let row = sqlx::query_as::<_, VpsPurpose>(
        r#"INSERT INTO vps_purposes (name, label, color, sort_order)
           VALUES ($1, $2, $3, $4)
           RETURNING *"#,
    )
    .bind(&input.name)
    .bind(&input.label)
    .bind(&input.color)
    .bind(input.sort_order)
    .fetch_one(&state.db)
    .await?;

    log_audit(
        &state.db,
        &user,
        "create",
        "vps_purpose",
        Some(&row.id.to_string()),
        serde_json::json!({"name": row.name, "label": row.label}),
    )
    .await;

    Ok(Json(row))
}

#[utoipa::path(
    put, path = "/api/vps-purposes/{id}",
    tag = "VPS Purposes",
    params(("id" = Uuid, Path, description = "VPS Purpose ID")),
    request_body = UpdateVpsPurpose,
    responses(
        (status = 200, body = VpsPurpose),
        (status = 404, body = ErrorResponse),
    )
)]
pub async fn update(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateVpsPurpose>,
) -> Result<Json<VpsPurpose>, AppError> {
    require_role(&user, &["admin", "operator"])?;
    let existing = sqlx::query_as::<_, VpsPurpose>("SELECT * FROM vps_purposes WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let old = serde_json::to_value(&existing).unwrap_or_default();

    let new_name = input.name.clone().unwrap_or_else(|| existing.name.clone());
    let new_label = input.label.unwrap_or(existing.label);
    let new_color = input.color.unwrap_or(existing.color);
    let new_sort_order = input.sort_order.unwrap_or(existing.sort_order);

    // If name changed, update all VPS records in a transaction
    let row = if new_name != existing.name {
        let mut tx = state.db.begin().await?;

        sqlx::query("UPDATE vps SET purpose = $1 WHERE purpose = $2")
            .bind(&new_name)
            .bind(&existing.name)
            .execute(&mut *tx)
            .await?;

        let row = sqlx::query_as::<_, VpsPurpose>(
            r#"UPDATE vps_purposes SET name = $2, label = $3, color = $4, sort_order = $5
               WHERE id = $1
               RETURNING *"#,
        )
        .bind(id)
        .bind(&new_name)
        .bind(&new_label)
        .bind(&new_color)
        .bind(new_sort_order)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        row
    } else {
        sqlx::query_as::<_, VpsPurpose>(
            r#"UPDATE vps_purposes SET name = $2, label = $3, color = $4, sort_order = $5
               WHERE id = $1
               RETURNING *"#,
        )
        .bind(id)
        .bind(&new_name)
        .bind(&new_label)
        .bind(&new_color)
        .bind(new_sort_order)
        .fetch_one(&state.db)
        .await?
    };

    let new_val = serde_json::to_value(&row).unwrap_or_default();
    let mut changes = serde_json::Map::new();
    let skip = ["id", "created_at", "updated_at"];
    if let (serde_json::Value::Object(old_map), serde_json::Value::Object(new_map)) =
        (&old, &new_val)
    {
        for (key, nv) in new_map {
            if skip.contains(&key.as_str()) {
                continue;
            }
            if let Some(ov) = old_map.get(key) {
                if ov != nv {
                    changes.insert(key.clone(), serde_json::json!({"from": ov, "to": nv}));
                }
            }
        }
    }

    log_audit(
        &state.db,
        &user,
        "update",
        "vps_purpose",
        Some(&id.to_string()),
        serde_json::json!({"name": row.name, "changes": changes}),
    )
    .await;

    Ok(Json(row))
}

#[utoipa::path(
    delete, path = "/api/vps-purposes/{id}",
    tag = "VPS Purposes",
    params(("id" = Uuid, Path, description = "VPS Purpose ID")),
    responses(
        (status = 200, description = "VPS Purpose deleted"),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
    )
)]
pub async fn delete(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    // Fetch the purpose to get its name
    let purpose = sqlx::query_as::<_, VpsPurpose>("SELECT * FROM vps_purposes WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Reject if any VPS uses this purpose
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM vps WHERE purpose = $1")
        .bind(&purpose.name)
        .fetch_one(&state.db)
        .await?;

    if count.0 > 0 {
        return Err(AppError::BadRequest(format!(
            "Cannot delete purpose '{}': {} VPS instance(s) still use it",
            purpose.name, count.0
        )));
    }

    let result = sqlx::query("DELETE FROM vps_purposes WHERE id = $1")
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
        "vps_purpose",
        Some(&id.to_string()),
        serde_json::json!({"name": purpose.name}),
    )
    .await;

    Ok(Json(serde_json::json!({ "deleted": true })))
}
