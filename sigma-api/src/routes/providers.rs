use axum::{
    extract::{Path, Query, State},
    http::header,
    response::IntoResponse,
    routing::get,
    Extension, Json, Router,
};
use uuid::Uuid;

use crate::auth::{require_role, CurrentUser};
use crate::errors::{AppError, ErrorResponse};
#[allow(unused_imports)]
use crate::models::PaginatedProviderResponse;
use crate::models::{
    CreateProvider, ExportQuery, ImportRequest, ImportResult, PaginatedResponse, Provider,
    ProviderCsvRow, ProviderListQuery, UpdateProvider,
};
use crate::routes::audit_logs::log_audit;
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/providers", get(list).post(create))
        .route("/api/providers/export", get(export))
        .route("/api/providers/import", axum::routing::post(import))
        .route(
            "/api/providers/{id}",
            get(get_one).put(update).delete(delete),
        )
}

#[utoipa::path(
    get, path = "/api/providers",
    tag = "Providers",
    params(ProviderListQuery),
    responses(
        (status = 200, body = PaginatedProviderResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    Query(q): Query<ProviderListQuery>,
) -> Result<Json<PaginatedResponse<Provider>>, AppError> {
    let per_page = q.per_page.clamp(1, 100);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM providers")
        .fetch_one(&state.db)
        .await?;

    let rows = sqlx::query_as::<_, Provider>(
        "SELECT * FROM providers ORDER BY name LIMIT $1 OFFSET $2",
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
    get, path = "/api/providers/{id}",
    tag = "Providers",
    params(("id" = Uuid, Path, description = "Provider ID")),
    responses(
        (status = 200, body = Provider),
        (status = 404, body = ErrorResponse),
    )
)]
pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Provider>, AppError> {
    let row = sqlx::query_as::<_, Provider>("SELECT * FROM providers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

#[utoipa::path(
    post, path = "/api/providers",
    tag = "Providers",
    request_body = CreateProvider,
    responses(
        (status = 200, body = Provider),
        (status = 400, body = ErrorResponse),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Json(input): Json<CreateProvider>,
) -> Result<Json<Provider>, AppError> {
    require_role(&user, &["admin", "operator"])?;
    let row = sqlx::query_as::<_, Provider>(
        r#"INSERT INTO providers (name, country, website, panel_url, api_supported, rating, notes)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING *"#,
    )
    .bind(&input.name)
    .bind(&input.country)
    .bind(&input.website)
    .bind(&input.panel_url)
    .bind(input.api_supported)
    .bind(input.rating)
    .bind(&input.notes)
    .fetch_one(&state.db)
    .await?;

    log_audit(&state.db, &user, "create", "provider", Some(&row.id.to_string()),
        serde_json::json!({"name": row.name})).await;

    Ok(Json(row))
}

#[utoipa::path(
    put, path = "/api/providers/{id}",
    tag = "Providers",
    params(("id" = Uuid, Path, description = "Provider ID")),
    request_body = UpdateProvider,
    responses(
        (status = 200, body = Provider),
        (status = 404, body = ErrorResponse),
    )
)]
pub async fn update(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateProvider>,
) -> Result<Json<Provider>, AppError> {
    require_role(&user, &["admin", "operator"])?;
    let existing = sqlx::query_as::<_, Provider>("SELECT * FROM providers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let old = serde_json::to_value(&existing).unwrap_or_default();

    let row = sqlx::query_as::<_, Provider>(
        r#"UPDATE providers SET
            name = $2, country = $3, website = $4, panel_url = $5,
            api_supported = $6, rating = $7, notes = $8
           WHERE id = $1
           RETURNING *"#,
    )
    .bind(id)
    .bind(input.name.unwrap_or(existing.name))
    .bind(input.country.unwrap_or(existing.country))
    .bind(input.website.unwrap_or(existing.website))
    .bind(input.panel_url.unwrap_or(existing.panel_url))
    .bind(input.api_supported.unwrap_or(existing.api_supported))
    .bind(input.rating.unwrap_or(existing.rating))
    .bind(input.notes.unwrap_or(existing.notes))
    .fetch_one(&state.db)
    .await?;

    let new = serde_json::to_value(&row).unwrap_or_default();
    let mut changes = serde_json::Map::new();
    let skip = ["id", "created_at", "updated_at"];
    if let (serde_json::Value::Object(old_map), serde_json::Value::Object(new_map)) = (&old, &new) {
        for (key, new_val) in new_map {
            if skip.contains(&key.as_str()) { continue; }
            if let Some(old_val) = old_map.get(key) {
                if old_val != new_val {
                    changes.insert(key.clone(), serde_json::json!({"from": old_val, "to": new_val}));
                }
            }
        }
    }

    log_audit(&state.db, &user, "update", "provider", Some(&id.to_string()),
        serde_json::json!({"name": row.name, "changes": changes})).await;

    Ok(Json(row))
}

#[utoipa::path(
    delete, path = "/api/providers/{id}",
    tag = "Providers",
    params(("id" = Uuid, Path, description = "Provider ID")),
    responses(
        (status = 200, description = "Provider deleted"),
        (status = 404, body = ErrorResponse),
    )
)]
pub async fn delete(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_role(&user, &["admin", "operator"])?;
    let result = sqlx::query("DELETE FROM providers WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    log_audit(&state.db, &user, "delete", "provider", Some(&id.to_string()),
        serde_json::json!({})).await;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

// ─── Export ──────────────────────────────────────────────

#[utoipa::path(
    get, path = "/api/providers/export",
    tag = "Providers",
    params(ExportQuery),
    responses(
        (status = 200, description = "Export data as CSV or JSON"),
    )
)]
pub async fn export(
    State(state): State<AppState>,
    Query(q): Query<ExportQuery>,
) -> Result<impl IntoResponse, AppError> {
    let rows = sqlx::query_as::<_, Provider>("SELECT * FROM providers ORDER BY name")
        .fetch_all(&state.db)
        .await?;

    match q.format.as_str() {
        "csv" => {
            let mut wtr = csv::Writer::from_writer(vec![]);
            for r in &rows {
                wtr.serialize(ProviderCsvRow {
                    name: r.name.clone(),
                    country: r.country.clone(),
                    website: r.website.clone(),
                    panel_url: r.panel_url.clone(),
                    api_supported: r.api_supported,
                    rating: r.rating,
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
                        "attachment; filename=\"providers.csv\"".to_string(),
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
                        "attachment; filename=\"providers.json\"".to_string(),
                    ),
                ],
                json,
            )
                .into_response())
        }
    }
}

// ─── Import ──────────────────────────────────────────────

#[utoipa::path(
    post, path = "/api/providers/import",
    tag = "Providers",
    request_body = ImportRequest,
    responses(
        (status = 200, body = ImportResult),
        (status = 400, body = ErrorResponse),
    )
)]
pub async fn import(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Json(input): Json<ImportRequest>,
) -> Result<Json<ImportResult>, AppError> {
    require_role(&user, &["admin", "operator"])?;
    let rows: Vec<ProviderCsvRow> = match input.format.as_str() {
        "csv" => {
            let mut rdr = csv::Reader::from_reader(input.data.as_bytes());
            let mut parsed = Vec::new();
            for (i, result) in rdr.deserialize().enumerate() {
                match result {
                    Ok(row) => parsed.push(row),
                    Err(e) => {
                        return Ok(Json(ImportResult {
                            imported: 0,
                            errors: vec![format!("Row {}: parse error: {e}", i + 1)],
                        }));
                    }
                }
            }
            parsed
        }
        "json" => serde_json::from_str::<Vec<ProviderCsvRow>>(&input.data)
            .map_err(|e| AppError::BadRequest(format!("Invalid JSON: {e}")))?,
        _ => return Err(AppError::BadRequest("format must be 'csv' or 'json'".into())),
    };

    let mut imported = 0usize;
    let mut errors = Vec::new();

    for (i, row) in rows.iter().enumerate() {
        let row_num = i + 1;
        if row.name.trim().is_empty() {
            errors.push(format!("Row {row_num}: name is required"));
            continue;
        }

        let result = sqlx::query(
            r#"INSERT INTO providers (name, country, website, panel_url, api_supported, rating, notes)
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        )
        .bind(&row.name)
        .bind(&row.country)
        .bind(&row.website)
        .bind(&row.panel_url)
        .bind(row.api_supported)
        .bind(row.rating)
        .bind(&row.notes)
        .execute(&state.db)
        .await;

        match result {
            Ok(_) => imported += 1,
            Err(e) => errors.push(format!("Row {row_num}: {e}")),
        }
    }

    Ok(Json(ImportResult { imported, errors }))
}
