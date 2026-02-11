use axum::{
    extract::{Path, Query, State},
    routing::get,
    Extension, Json, Router,
};
use uuid::Uuid;

use crate::auth::{require_role, CurrentUser};
use crate::errors::{AppError, ErrorResponse};
#[allow(unused_imports)]
use crate::models::PaginatedExchangeRateResponse;
use crate::models::{
    CreateExchangeRate, ExchangeRate, ExchangeRateListQuery, PaginatedResponse,
    UpdateExchangeRate,
};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/exchange-rates", get(list).post(create))
        .route(
            "/api/exchange-rates/{id}",
            get(get_one).put(update).delete(delete),
        )
}

#[utoipa::path(
    get, path = "/api/exchange-rates",
    tag = "Exchange Rates",
    params(ExchangeRateListQuery),
    responses(
        (status = 200, body = PaginatedExchangeRateResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    Query(q): Query<ExchangeRateListQuery>,
) -> Result<Json<PaginatedResponse<ExchangeRate>>, AppError> {
    let per_page = q.per_page.clamp(1, 100);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let mut where_clause = String::from(" WHERE 1=1");
    let mut param_idx = 0u32;

    if q.from_currency.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND from_currency = ${}", param_idx));
    }
    if q.to_currency.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND to_currency = ${}", param_idx));
    }

    let count_sql = format!("SELECT COUNT(*) FROM exchange_rates{}", where_clause);
    let mut count_query = sqlx::query_as::<_, (i64,)>(&count_sql);
    if let Some(ref v) = q.from_currency {
        count_query = count_query.bind(v);
    }
    if let Some(ref v) = q.to_currency {
        count_query = count_query.bind(v);
    }
    let total = count_query.fetch_one(&state.db).await?.0;

    param_idx += 1;
    let limit_param = param_idx;
    param_idx += 1;
    let offset_param = param_idx;

    let select_sql = format!(
        "SELECT * FROM exchange_rates{} ORDER BY from_currency, to_currency LIMIT ${} OFFSET ${}",
        where_clause, limit_param, offset_param
    );
    let mut select_query = sqlx::query_as::<_, ExchangeRate>(&select_sql);
    if let Some(ref v) = q.from_currency {
        select_query = select_query.bind(v);
    }
    if let Some(ref v) = q.to_currency {
        select_query = select_query.bind(v);
    }
    select_query = select_query.bind(per_page).bind(offset);

    let rows = select_query.fetch_all(&state.db).await?;

    Ok(Json(PaginatedResponse {
        data: rows,
        total,
        page,
        per_page,
    }))
}

#[utoipa::path(
    get, path = "/api/exchange-rates/{id}",
    tag = "Exchange Rates",
    params(("id" = Uuid, Path, description = "Exchange rate ID")),
    responses(
        (status = 200, body = ExchangeRate),
        (status = 404, body = ErrorResponse),
    )
)]
pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ExchangeRate>, AppError> {
    let row = sqlx::query_as::<_, ExchangeRate>("SELECT * FROM exchange_rates WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

#[utoipa::path(
    post, path = "/api/exchange-rates",
    tag = "Exchange Rates",
    request_body = CreateExchangeRate,
    responses(
        (status = 200, body = ExchangeRate),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Json(body): Json<CreateExchangeRate>,
) -> Result<Json<ExchangeRate>, AppError> {
    require_role(&user, &["admin", "operator"])?;
    let row = sqlx::query_as::<_, ExchangeRate>(
        "INSERT INTO exchange_rates (from_currency, to_currency, rate)
         VALUES ($1, $2, $3)
         ON CONFLICT (from_currency, to_currency) DO UPDATE
         SET rate = EXCLUDED.rate, updated_at = NOW()
         RETURNING *",
    )
    .bind(&body.from_currency)
    .bind(&body.to_currency)
    .bind(body.rate)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(row))
}

#[utoipa::path(
    put, path = "/api/exchange-rates/{id}",
    tag = "Exchange Rates",
    params(("id" = Uuid, Path, description = "Exchange rate ID")),
    request_body = UpdateExchangeRate,
    responses(
        (status = 200, body = ExchangeRate),
        (status = 404, body = ErrorResponse),
    )
)]
pub async fn update(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateExchangeRate>,
) -> Result<Json<ExchangeRate>, AppError> {
    require_role(&user, &["admin", "operator"])?;
    let row = sqlx::query_as::<_, ExchangeRate>(
        "UPDATE exchange_rates SET rate = $1, updated_at = NOW() WHERE id = $2 RETURNING *",
    )
    .bind(body.rate)
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(row))
}

#[utoipa::path(
    delete, path = "/api/exchange-rates/{id}",
    tag = "Exchange Rates",
    params(("id" = Uuid, Path, description = "Exchange rate ID")),
    responses(
        (status = 200, description = "Deleted"),
        (status = 404, body = ErrorResponse),
    )
)]
pub async fn delete(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    let result = sqlx::query("DELETE FROM exchange_rates WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(serde_json::json!({"deleted": true})))
}
