use axum::{extract::State, routing::{get, post}, Extension, Json, Router};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::auth::{require_role, CurrentUser};
use crate::errors::AppError;
use crate::models::{
    DuplicateDetectionResponse, DuplicateGroup, IpEntry, MergeVpsRequest, MergeVpsResponse, Vps,
};
use crate::routes::audit_logs::log_audit;
use crate::routes::cloud::merge_ip_labels_union;
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/vps/duplicates", get(detect_duplicates))
        .route("/api/vps/merge", post(merge_vps))
}

// ─── Detect Duplicates ────────────────────────────────────

#[derive(sqlx::FromRow)]
struct DuplicatePairRow {
    vps_a_id: Uuid,
    vps_b_id: Uuid,
    shared_ips: Vec<String>,
}

#[utoipa::path(
    get,
    path = "/api/vps/duplicates",
    tag = "VPS",
    responses(
        (status = 200, body = DuplicateDetectionResponse),
    )
)]
pub async fn detect_duplicates(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
) -> Result<Json<DuplicateDetectionResponse>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    let pairs = sqlx::query_as::<_, DuplicatePairRow>(
        r#"
        WITH public_ips AS (
            SELECT v.id AS vps_id, e->>'ip' AS ip
            FROM vps v, jsonb_array_elements(v.ip_addresses) AS e
            WHERE v.status NOT IN ('retired', 'deleted')
              AND e->>'label' != 'internal'
              AND e->>'ip' IS NOT NULL
        )
        SELECT
            a.vps_id AS vps_a_id,
            b.vps_id AS vps_b_id,
            array_agg(DISTINCT a.ip) AS shared_ips
        FROM public_ips a
        JOIN public_ips b ON a.ip = b.ip AND a.vps_id < b.vps_id
        GROUP BY a.vps_id, b.vps_id
        ORDER BY a.vps_id, b.vps_id
        "#,
    )
    .fetch_all(&state.db)
    .await?;

    if pairs.is_empty() {
        return Ok(Json(DuplicateDetectionResponse {
            groups: vec![],
            total_groups: 0,
        }));
    }

    // Collect all VPS IDs we need
    let mut vps_ids: HashSet<Uuid> = HashSet::new();
    for p in &pairs {
        vps_ids.insert(p.vps_a_id);
        vps_ids.insert(p.vps_b_id);
    }
    let vps_ids_vec: Vec<Uuid> = vps_ids.into_iter().collect();

    let vps_rows = sqlx::query_as::<_, Vps>(
        "SELECT * FROM vps WHERE id = ANY($1)",
    )
    .bind(&vps_ids_vec)
    .fetch_all(&state.db)
    .await?;

    let vps_map: HashMap<Uuid, Vps> = vps_rows.into_iter().map(|v| (v.id, v)).collect();

    let mut groups = Vec::with_capacity(pairs.len());
    for pair in pairs {
        if let (Some(vps_a), Some(vps_b)) = (
            vps_map.get(&pair.vps_a_id),
            vps_map.get(&pair.vps_b_id),
        ) {
            // We need owned copies since Vps doesn't implement Clone;
            // re-fetch via query would be wasteful, so serialize/deserialize
            let vps_a_json = serde_json::to_value(vps_a).unwrap();
            let vps_b_json = serde_json::to_value(vps_b).unwrap();
            groups.push(DuplicateGroup {
                vps_a: serde_json::from_value(vps_a_json).unwrap(),
                vps_b: serde_json::from_value(vps_b_json).unwrap(),
                shared_ips: pair.shared_ips,
            });
        }
    }

    let total_groups = groups.len();
    Ok(Json(DuplicateDetectionResponse {
        groups,
        total_groups,
    }))
}

// ─── Merge VPS ────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/api/vps/merge",
    tag = "VPS",
    request_body = MergeVpsRequest,
    responses(
        (status = 200, body = MergeVpsResponse),
        (status = 400),
        (status = 404),
    )
)]
pub async fn merge_vps(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Json(input): Json<MergeVpsRequest>,
) -> Result<Json<MergeVpsResponse>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    if input.target_id == input.source_id {
        return Err(AppError::BadRequest(
            "target_id and source_id must be different".into(),
        ));
    }

    let mut tx = state.db.begin().await?;

    // 1. Fetch both VPS records
    let target = sqlx::query_as::<_, Vps>("SELECT * FROM vps WHERE id = $1 FOR UPDATE")
        .bind(input.target_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::BadRequest("Target VPS not found".into()))?;

    let source = sqlx::query_as::<_, Vps>("SELECT * FROM vps WHERE id = $1 FOR UPDATE")
        .bind(input.source_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::BadRequest("Source VPS not found".into()))?;

    // 2. Build merged fields
    let target_ips: Vec<IpEntry> = target.ip_addresses.0.clone();
    let source_ips: Vec<IpEntry> = source.ip_addresses.0.clone();
    let merged_ips = merge_ip_labels_union(&source_ips, &target_ips);

    // Extra: target values win (top-level object merge)
    let merged_extra = {
        let mut base = match source.extra.clone() {
            serde_json::Value::Object(m) => m,
            _ => serde_json::Map::new(),
        };
        if let serde_json::Value::Object(t) = target.extra.clone() {
            for (k, v) in t {
                base.insert(k, v);
            }
        }
        // Preserve source hostname so agent heartbeat can still match
        if source.hostname != target.hostname {
            let mut hostnames: Vec<String> = base
                .get("merged_hostnames")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            if !hostnames.contains(&source.hostname) {
                hostnames.push(source.hostname.clone());
            }
            base.insert(
                "merged_hostnames".to_string(),
                serde_json::Value::Array(hostnames.into_iter().map(serde_json::Value::String).collect()),
            );
        }
        serde_json::Value::Object(base)
    };

    // Tags union
    let merged_tags = {
        let mut set: HashSet<String> = target.tags.iter().cloned().collect();
        set.extend(source.tags.iter().cloned());
        let mut tags: Vec<String> = set.into_iter().collect();
        tags.sort();
        tags
    };

    // Non-empty fallback for string fields
    let country = if target.country.is_empty() { &source.country } else { &target.country };
    let city = if target.city.is_empty() { &source.city } else { &target.city };
    let dc_name = if target.dc_name.is_empty() { &source.dc_name } else { &target.dc_name };
    let purpose = if target.purpose.is_empty() { &source.purpose } else { &target.purpose };
    let alias = if target.alias.is_empty() { &source.alias } else { &target.alias };
    let notes = if target.notes.is_empty() { &source.notes } else { &target.notes };

    // FK fallbacks
    let provider_id = target.provider_id.or(source.provider_id);
    let cloud_account_id = target.cloud_account_id.or(source.cloud_account_id);

    // Numeric fallbacks
    let cpu_cores = target.cpu_cores.or(source.cpu_cores);
    let ram_mb = target.ram_mb.or(source.ram_mb);
    let disk_gb = target.disk_gb.or(source.disk_gb);
    let bandwidth_tb = target.bandwidth_tb.or(source.bandwidth_tb);
    let cost_monthly = target.cost_monthly.or(source.cost_monthly);

    // 3. Reassign FK references from source → target
    sqlx::query("UPDATE ip_checks SET vps_id = $1 WHERE vps_id = $2")
        .bind(input.target_id)
        .bind(input.source_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("UPDATE vps_ip_history SET vps_id = $1 WHERE vps_id = $2")
        .bind(input.target_id)
        .bind(input.source_id)
        .execute(&mut *tx)
        .await?;

    // envoy_nodes: delete conflicts (same node_id on target), then reassign
    sqlx::query(
        r#"DELETE FROM envoy_nodes
           WHERE vps_id = $2
             AND node_id IN (SELECT node_id FROM envoy_nodes WHERE vps_id = $1)"#,
    )
    .bind(input.target_id)
    .bind(input.source_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE envoy_nodes SET vps_id = $1 WHERE vps_id = $2")
        .bind(input.target_id)
        .bind(input.source_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("UPDATE tickets SET vps_id = $1 WHERE vps_id = $2")
        .bind(input.target_id)
        .bind(input.source_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("UPDATE dns_records SET vps_id = $1 WHERE vps_id = $2")
        .bind(input.target_id)
        .bind(input.source_id)
        .execute(&mut *tx)
        .await?;

    // 4. UPDATE target with merged fields
    let merged_ips_json = serde_json::to_value(&merged_ips)
        .map_err(|e| AppError::Internal(format!("Failed to serialize merged IPs: {e}")))?;

    let merged_vps = sqlx::query_as::<_, Vps>(
        r#"UPDATE vps SET
            ip_addresses = $2,
            extra = $3,
            tags = $4,
            country = $5,
            city = $6,
            dc_name = $7,
            purpose = $8,
            alias = $9,
            notes = $10,
            provider_id = $11,
            cloud_account_id = $12,
            cpu_cores = $13,
            ram_mb = $14,
            disk_gb = $15,
            bandwidth_tb = $16,
            cost_monthly = $17,
            updated_at = now()
        WHERE id = $1
        RETURNING *"#,
    )
    .bind(input.target_id)
    .bind(merged_ips_json)
    .bind(&merged_extra)
    .bind(&merged_tags)
    .bind(country)
    .bind(city)
    .bind(dc_name)
    .bind(purpose)
    .bind(alias)
    .bind(notes)
    .bind(provider_id)
    .bind(cloud_account_id)
    .bind(cpu_cores)
    .bind(ram_mb)
    .bind(disk_gb)
    .bind(bandwidth_tb)
    .bind(cost_monthly)
    .fetch_one(&mut *tx)
    .await?;

    // 5. Soft-delete source VPS
    sqlx::query("UPDATE vps SET status = 'deleted', updated_at = now() WHERE id = $1")
        .bind(input.source_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    // 6. Audit log the merge
    log_audit(
        &state.db,
        &user,
        "merge",
        "vps",
        Some(&input.target_id.to_string()),
        serde_json::json!({
            "target_id": input.target_id,
            "source_id": input.source_id,
            "source_hostname": source.hostname,
            "target_hostname": target.hostname,
        }),
    )
    .await;

    Ok(Json(MergeVpsResponse {
        merged_vps,
        deleted_id: input.source_id,
    }))
}
