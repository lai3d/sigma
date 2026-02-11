use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{CreateProvider, Provider, UpdateProvider};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/providers", get(list).post(create))
        .route("/api/providers/{id}", get(get_one).put(update).delete(delete))
}

async fn list(State(state): State<AppState>) -> Result<Json<Vec<Provider>>, AppError> {
    let rows = sqlx::query_as::<_, Provider>("SELECT * FROM providers ORDER BY name")
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows))
}

async fn get_one(
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

async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateProvider>,
) -> Result<Json<Provider>, AppError> {
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

    Ok(Json(row))
}

async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateProvider>,
) -> Result<Json<Provider>, AppError> {
    let existing = sqlx::query_as::<_, Provider>("SELECT * FROM providers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

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

    Ok(Json(row))
}

async fn delete(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let result = sqlx::query("DELETE FROM providers WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(serde_json::json!({ "deleted": true })))
}
