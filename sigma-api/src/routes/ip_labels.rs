use axum::{
    extract::{Path, Query, State},
    routing::get,
    Extension, Json, Router,
};
use uuid::Uuid;

use crate::auth::{require_role, CurrentUser};
use crate::errors::{AppError, ErrorResponse};
#[allow(unused_imports)]
use crate::models::PaginatedIpLabelResponse;
use crate::models::{
    CreateIpLabel, IpLabel, IpLabelListQuery, PaginatedResponse, UpdateIpLabel,
};
use crate::routes::audit_logs::log_audit;
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/ip-labels", get(list).post(create))
        .route(
            "/api/ip-labels/{id}",
            get(get_one).put(update).delete(delete),
        )
}

#[utoipa::path(
    get, path = "/api/ip-labels",
    tag = "IP Labels",
    params(IpLabelListQuery),
    responses(
        (status = 200, body = PaginatedIpLabelResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    Query(q): Query<IpLabelListQuery>,
) -> Result<Json<PaginatedResponse<IpLabel>>, AppError> {
    let per_page = q.per_page.clamp(1, 100);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM ip_labels")
        .fetch_one(&state.db)
        .await?;

    let rows = sqlx::query_as::<_, IpLabel>(
        "SELECT * FROM ip_labels ORDER BY sort_order, name LIMIT $1 OFFSET $2",
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
    get, path = "/api/ip-labels/{id}",
    tag = "IP Labels",
    params(("id" = Uuid, Path, description = "IP Label ID")),
    responses(
        (status = 200, body = IpLabel),
        (status = 404, body = ErrorResponse),
    )
)]
pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<IpLabel>, AppError> {
    let row = sqlx::query_as::<_, IpLabel>("SELECT * FROM ip_labels WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

#[utoipa::path(
    post, path = "/api/ip-labels",
    tag = "IP Labels",
    request_body = CreateIpLabel,
    responses(
        (status = 200, body = IpLabel),
        (status = 400, body = ErrorResponse),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Json(input): Json<CreateIpLabel>,
) -> Result<Json<IpLabel>, AppError> {
    require_role(&user, &["admin", "operator"])?;
    let row = sqlx::query_as::<_, IpLabel>(
        r#"INSERT INTO ip_labels (name, label, short, color, sort_order)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING *"#,
    )
    .bind(&input.name)
    .bind(&input.label)
    .bind(&input.short)
    .bind(&input.color)
    .bind(input.sort_order)
    .fetch_one(&state.db)
    .await?;

    log_audit(
        &state.db,
        &user,
        "create",
        "ip_label",
        Some(&row.id.to_string()),
        serde_json::json!({"name": row.name, "label": row.label}),
    )
    .await;

    Ok(Json(row))
}

#[utoipa::path(
    put, path = "/api/ip-labels/{id}",
    tag = "IP Labels",
    params(("id" = Uuid, Path, description = "IP Label ID")),
    request_body = UpdateIpLabel,
    responses(
        (status = 200, body = IpLabel),
        (status = 404, body = ErrorResponse),
    )
)]
pub async fn update(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateIpLabel>,
) -> Result<Json<IpLabel>, AppError> {
    require_role(&user, &["admin", "operator"])?;
    let existing = sqlx::query_as::<_, IpLabel>("SELECT * FROM ip_labels WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let old = serde_json::to_value(&existing).unwrap_or_default();

    let new_name = input.name.clone().unwrap_or_else(|| existing.name.clone());
    let new_label = input.label.unwrap_or(existing.label);
    let new_short = input.short.unwrap_or(existing.short);
    let new_color = input.color.unwrap_or(existing.color);
    let new_sort_order = input.sort_order.unwrap_or(existing.sort_order);

    // If name changed, cascade update all ip_addresses JSONB labels in vps table
    let row = if new_name != existing.name {
        let mut tx = state.db.begin().await?;

        // Update label references inside ip_addresses JSONB arrays across all VPS records
        sqlx::query(
            r#"UPDATE vps SET ip_addresses = (
                SELECT COALESCE(jsonb_agg(
                    CASE WHEN elem->>'label' = $2
                         THEN jsonb_set(elem, '{label}', to_jsonb($1::text))
                         ELSE elem
                    END
                ), '[]'::jsonb)
                FROM jsonb_array_elements(ip_addresses) AS elem
            )
            WHERE ip_addresses @> ANY(ARRAY[
                jsonb_build_array(jsonb_build_object('label', $2::text))
            ]::jsonb[])"#,
        )
        .bind(&new_name)
        .bind(&existing.name)
        .execute(&mut *tx)
        .await?;

        let row = sqlx::query_as::<_, IpLabel>(
            r#"UPDATE ip_labels SET name = $2, label = $3, short = $4, color = $5, sort_order = $6
               WHERE id = $1
               RETURNING *"#,
        )
        .bind(id)
        .bind(&new_name)
        .bind(&new_label)
        .bind(&new_short)
        .bind(&new_color)
        .bind(new_sort_order)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        row
    } else {
        sqlx::query_as::<_, IpLabel>(
            r#"UPDATE ip_labels SET name = $2, label = $3, short = $4, color = $5, sort_order = $6
               WHERE id = $1
               RETURNING *"#,
        )
        .bind(id)
        .bind(&new_name)
        .bind(&new_label)
        .bind(&new_short)
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
        "ip_label",
        Some(&id.to_string()),
        serde_json::json!({"name": row.name, "changes": changes}),
    )
    .await;

    Ok(Json(row))
}

#[utoipa::path(
    delete, path = "/api/ip-labels/{id}",
    tag = "IP Labels",
    params(("id" = Uuid, Path, description = "IP Label ID")),
    responses(
        (status = 200, description = "IP Label deleted"),
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

    let label = sqlx::query_as::<_, IpLabel>("SELECT * FROM ip_labels WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Reject if any VPS IP uses this label
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM vps WHERE ip_addresses @> ANY(ARRAY[jsonb_build_array(jsonb_build_object('label', $1::text))]::jsonb[])",
    )
    .bind(&label.name)
    .fetch_one(&state.db)
    .await?;

    if count.0 > 0 {
        return Err(AppError::BadRequest(format!(
            "Cannot delete label '{}': {} VPS instance(s) still use it",
            label.name, count.0
        )));
    }

    let result = sqlx::query("DELETE FROM ip_labels WHERE id = $1")
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
        "ip_label",
        Some(&id.to_string()),
        serde_json::json!({"name": label.name}),
    )
    .await;

    Ok(Json(serde_json::json!({ "deleted": true })))
}
