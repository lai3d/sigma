use axum::{extract::State, routing::get, Json, Router};

use crate::errors::AppError;
use crate::models::{CountStat, DashboardStats, Vps};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/stats", get(dashboard))
}

async fn dashboard(State(state): State<AppState>) -> Result<Json<DashboardStats>, AppError> {
    let total_vps: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM vps")
        .fetch_one(&state.db)
        .await?;

    let active_vps: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM vps WHERE status = 'active'")
            .fetch_one(&state.db)
            .await?;

    let total_providers: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM providers")
        .fetch_one(&state.db)
        .await?;

    let by_country = sqlx::query_as::<_, CountStat>(
        "SELECT country as label, COUNT(*) as count FROM vps WHERE status != 'retired' GROUP BY country ORDER BY count DESC",
    )
    .fetch_all(&state.db)
    .await?;

    let by_provider = sqlx::query_as::<_, CountStat>(
        r#"SELECT p.name as label, COUNT(*) as count
           FROM vps v JOIN providers p ON p.id = v.provider_id
           WHERE v.status != 'retired'
           GROUP BY p.name ORDER BY count DESC"#,
    )
    .fetch_all(&state.db)
    .await?;

    let by_status = sqlx::query_as::<_, CountStat>(
        "SELECT status as label, COUNT(*) as count FROM vps GROUP BY status ORDER BY count DESC",
    )
    .fetch_all(&state.db)
    .await?;

    let expiring_soon = sqlx::query_as::<_, Vps>(
        r#"SELECT * FROM vps
           WHERE expire_date IS NOT NULL
             AND expire_date <= CURRENT_DATE + INTERVAL '14 days'
             AND status IN ('active', 'provisioning')
           ORDER BY expire_date ASC"#,
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(DashboardStats {
        total_vps: total_vps.0,
        active_vps: active_vps.0,
        total_providers: total_providers.0,
        by_country,
        by_provider,
        by_status,
        expiring_soon,
    }))
}
