use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use rust_decimal::Decimal;

use crate::errors::{AppError, ErrorResponse};
use crate::models::{
    ConvertedTotal, CostMonthlyQuery, CostMonthlyResponse, CostSummaryQuery, CostSummaryResponse,
    CostSummaryRow, CurrencyBreakdown, MonthlyCostEntry, MonthlyCostRow,
};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/costs/summary", get(summary))
        .route("/api/costs/monthly", get(monthly))
}

#[utoipa::path(
    get, path = "/api/costs/summary",
    tag = "Costs",
    params(CostSummaryQuery),
    responses(
        (status = 200, body = CostSummaryResponse),
        (status = 400, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn summary(
    State(state): State<AppState>,
    Query(q): Query<CostSummaryQuery>,
) -> Result<Json<CostSummaryResponse>, AppError> {
    let mut where_parts: Vec<String> = vec!["cost_monthly IS NOT NULL".to_string()];
    let mut bindings: Vec<QueryBinding> = Vec::new();

    if let Some(ref status) = q.status {
        bindings.push(QueryBinding::Text(status.clone()));
        where_parts.push(format!("status = ${}", bindings.len()));
    } else {
        where_parts.push("status != 'retired'".to_string());
    }

    if let Some(ref provider_id) = q.provider_id {
        bindings.push(QueryBinding::Uuid(*provider_id));
        where_parts.push(format!("provider_id = ${}", bindings.len()));
    }

    if let Some(ref country) = q.country {
        bindings.push(QueryBinding::Text(country.clone()));
        where_parts.push(format!("country = ${}", bindings.len()));
    }

    let where_clause = format!(" WHERE {}", where_parts.join(" AND "));
    let sql = format!(
        "SELECT currency, COUNT(*) as vps_count, COALESCE(SUM(cost_monthly), 0) as total_cost FROM vps{} GROUP BY currency ORDER BY currency",
        where_clause
    );

    let mut query = sqlx::query_as::<_, CostSummaryRow>(&sql);
    for binding in &bindings {
        match binding {
            QueryBinding::Text(v) => query = query.bind(v),
            QueryBinding::Uuid(v) => query = query.bind(v),
            QueryBinding::Int(v) => query = query.bind(v),
        }
    }

    let rows = query.fetch_all(&state.db).await?;

    let per_currency: Vec<CurrencyBreakdown> = rows
        .iter()
        .map(|r| CurrencyBreakdown {
            currency: r.currency.clone(),
            vps_count: r.vps_count,
            total_cost: r.total_cost,
        })
        .collect();

    let converted_total = if let Some(ref target_currency) = q.convert_to {
        Some(convert_totals(&state, &per_currency, target_currency).await?)
    } else {
        None
    };

    Ok(Json(CostSummaryResponse {
        per_currency,
        converted_total,
    }))
}

#[utoipa::path(
    get, path = "/api/costs/monthly",
    tag = "Costs",
    params(CostMonthlyQuery),
    responses(
        (status = 200, body = CostMonthlyResponse),
        (status = 400, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn monthly(
    State(state): State<AppState>,
    Query(q): Query<CostMonthlyQuery>,
) -> Result<Json<CostMonthlyResponse>, AppError> {
    let months = q.months.unwrap_or(12).clamp(1, 120);

    let mut where_parts: Vec<String> = vec![
        "v.cost_monthly IS NOT NULL".to_string(),
        "v.status != 'retired'".to_string(),
    ];
    let mut bindings: Vec<QueryBinding> = Vec::new();

    // $1 is months
    bindings.push(QueryBinding::Int(months));

    if let Some(ref provider_id) = q.provider_id {
        bindings.push(QueryBinding::Uuid(*provider_id));
        where_parts.push(format!("v.provider_id = ${}", bindings.len()));
    }

    if let Some(ref country) = q.country {
        bindings.push(QueryBinding::Text(country.clone()));
        where_parts.push(format!("v.country = ${}", bindings.len()));
    }

    let where_clause = where_parts.join(" AND ");

    let sql = format!(
        r#"
        WITH months AS (
            SELECT date_trunc('month', NOW() - (n || ' months')::interval)::date AS month
            FROM generate_series(0, $1 - 1) AS n
        )
        SELECT
            m.month,
            v.currency,
            COALESCE(SUM(v.cost_monthly), 0) AS total_cost
        FROM months m
        LEFT JOIN vps v ON
            {} AND
            (v.purchase_date IS NULL OR v.purchase_date <= (m.month + interval '1 month' - interval '1 day')::date) AND
            (v.expire_date IS NULL OR v.expire_date >= m.month)
        GROUP BY m.month, v.currency
        ORDER BY m.month DESC, v.currency
        "#,
        where_clause
    );

    let mut query = sqlx::query_as::<_, MonthlyCostRow>(&sql);
    for binding in &bindings {
        match binding {
            QueryBinding::Int(v) => query = query.bind(v),
            QueryBinding::Text(v) => query = query.bind(v),
            QueryBinding::Uuid(v) => query = query.bind(v),
        }
    }

    let rows = query.fetch_all(&state.db).await?;

    // Group rows by month
    let mut month_map: std::collections::BTreeMap<chrono::NaiveDate, Vec<CurrencyBreakdown>> =
        std::collections::BTreeMap::new();

    for row in &rows {
        if let Some(ref currency) = row.currency {
            month_map
                .entry(row.month)
                .or_default()
                .push(CurrencyBreakdown {
                    currency: currency.clone(),
                    vps_count: 0, // monthly view doesn't track vps_count per-currency
                    total_cost: row.total_cost,
                });
        }
    }

    let mut months_result: Vec<MonthlyCostEntry> = Vec::new();
    for (month, per_currency) in &month_map {
        let converted_total = if let Some(ref target_currency) = q.convert_to {
            Some(convert_totals(&state, per_currency, target_currency).await?)
        } else {
            None
        };
        months_result.push(MonthlyCostEntry {
            month: *month,
            per_currency: per_currency.clone(),
            converted_total,
        });
    }

    // Sort descending by month
    months_result.sort_by(|a, b| b.month.cmp(&a.month));

    Ok(Json(CostMonthlyResponse {
        months: months_result,
    }))
}

// ─── Helpers ─────────────────────────────────────────────

enum QueryBinding {
    Text(String),
    Uuid(uuid::Uuid),
    Int(i32),
}

async fn convert_totals(
    state: &AppState,
    per_currency: &[CurrencyBreakdown],
    target_currency: &str,
) -> Result<ConvertedTotal, AppError> {
    let rates = sqlx::query_as::<_, (String, Decimal)>(
        "SELECT from_currency, rate FROM exchange_rates WHERE to_currency = $1",
    )
    .bind(target_currency)
    .fetch_all(&state.db)
    .await?;

    let rate_map: std::collections::HashMap<String, Decimal> =
        rates.into_iter().collect();

    let mut total = Decimal::ZERO;
    for entry in per_currency {
        if entry.currency == target_currency {
            total += entry.total_cost;
        } else if let Some(rate) = rate_map.get(&entry.currency) {
            total += entry.total_cost * rate;
        } else {
            return Err(AppError::BadRequest(format!(
                "No exchange rate found for {} -> {}",
                entry.currency, target_currency
            )));
        }
    }

    Ok(ConvertedTotal {
        currency: target_currency.to_string(),
        amount: total,
    })
}
