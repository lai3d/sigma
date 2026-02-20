use axum::{
    extract::{Path, Query, State},
    routing::get,
    Extension, Json, Router,
};
use uuid::Uuid;

use crate::auth::{require_role, CurrentUser};
use crate::errors::AppError;
use crate::models::{
    CreateEnvoyNode, CreateEnvoyRoute, EnvoyNode, EnvoyNodeListQuery, EnvoyRoute,
    EnvoyRouteListQuery, PaginatedEnvoyNodeResponse, PaginatedEnvoyRouteResponse,
    PaginatedResponse, UpdateEnvoyNode, UpdateEnvoyRoute,
};
use crate::routes::audit_logs::log_audit;
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/envoy-nodes", get(list_nodes).post(create_node))
        .route(
            "/api/envoy-nodes/{id}",
            get(get_node).put(update_node).delete(delete_node),
        )
        .route("/api/envoy-routes", get(list_routes).post(create_route))
        .route(
            "/api/envoy-routes/{id}",
            get(get_route).put(update_route).delete(delete_route),
        )
}

// ─── Envoy Nodes ─────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/envoy-nodes",
    tag = "Envoy",
    params(EnvoyNodeListQuery),
    responses(
        (status = 200, body = PaginatedEnvoyNodeResponse),
    )
)]
pub async fn list_nodes(
    State(state): State<AppState>,
    Query(q): Query<EnvoyNodeListQuery>,
) -> Result<Json<PaginatedResponse<EnvoyNode>>, AppError> {
    let per_page = q.per_page.clamp(1, 100);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let mut where_clause = String::from(" WHERE 1=1");
    let mut param_idx = 0u32;

    if q.vps_id.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND vps_id = ${param_idx}"));
    }
    if q.status.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND status = ${param_idx}"));
    }

    // Count
    let count_sql = format!("SELECT COUNT(*) FROM envoy_nodes{where_clause}");
    let mut count_query = sqlx::query_as::<_, (i64,)>(&count_sql);
    if let Some(ref v) = q.vps_id {
        count_query = count_query.bind(v);
    }
    if let Some(ref v) = q.status {
        count_query = count_query.bind(v);
    }
    let total = count_query.fetch_one(&state.db).await?.0;

    // Data
    param_idx += 1;
    let limit_param = param_idx;
    param_idx += 1;
    let offset_param = param_idx;

    let data_sql = format!(
        "SELECT * FROM envoy_nodes{where_clause} ORDER BY created_at DESC LIMIT ${limit_param} OFFSET ${offset_param}"
    );
    let mut query = sqlx::query_as::<_, EnvoyNode>(&data_sql);
    if let Some(ref v) = q.vps_id {
        query = query.bind(v);
    }
    if let Some(ref v) = q.status {
        query = query.bind(v);
    }
    query = query.bind(per_page).bind(offset);

    let rows = query.fetch_all(&state.db).await?;

    Ok(Json(PaginatedResponse {
        data: rows,
        total,
        page,
        per_page,
    }))
}

#[utoipa::path(
    get,
    path = "/api/envoy-nodes/{id}",
    tag = "Envoy",
    params(("id" = Uuid, Path, description = "Envoy Node ID")),
    responses(
        (status = 200, body = EnvoyNode),
        (status = 404),
    )
)]
pub async fn get_node(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<EnvoyNode>, AppError> {
    let row = sqlx::query_as::<_, EnvoyNode>("SELECT * FROM envoy_nodes WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(row))
}

#[utoipa::path(
    post,
    path = "/api/envoy-nodes",
    tag = "Envoy",
    request_body = CreateEnvoyNode,
    responses(
        (status = 200, body = EnvoyNode),
    )
)]
pub async fn create_node(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Json(input): Json<CreateEnvoyNode>,
) -> Result<Json<EnvoyNode>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    // Validate vps_id exists
    sqlx::query_as::<_, (Uuid,)>("SELECT id FROM vps WHERE id = $1")
        .bind(input.vps_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("VPS {} not found", input.vps_id)))?;

    let row = sqlx::query_as::<_, EnvoyNode>(
        r#"INSERT INTO envoy_nodes (vps_id, node_id, admin_port, description, status)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING *"#,
    )
    .bind(input.vps_id)
    .bind(&input.node_id)
    .bind(input.admin_port)
    .bind(&input.description)
    .bind(&input.status)
    .fetch_one(&state.db)
    .await?;

    log_audit(
        &state.db,
        &user,
        "create",
        "envoy_node",
        Some(&row.id.to_string()),
        serde_json::json!({"node_id": row.node_id, "vps_id": row.vps_id.to_string()}),
    )
    .await;

    Ok(Json(row))
}

#[utoipa::path(
    put,
    path = "/api/envoy-nodes/{id}",
    tag = "Envoy",
    params(("id" = Uuid, Path, description = "Envoy Node ID")),
    request_body = UpdateEnvoyNode,
    responses(
        (status = 200, body = EnvoyNode),
        (status = 404),
    )
)]
pub async fn update_node(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateEnvoyNode>,
) -> Result<Json<EnvoyNode>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    let existing = sqlx::query_as::<_, EnvoyNode>("SELECT * FROM envoy_nodes WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let old = serde_json::to_value(&existing).unwrap_or_default();

    let row = sqlx::query_as::<_, EnvoyNode>(
        r#"UPDATE envoy_nodes SET
            node_id = $2, admin_port = $3, description = $4, status = $5, updated_at = now()
           WHERE id = $1
           RETURNING *"#,
    )
    .bind(id)
    .bind(input.node_id.unwrap_or(existing.node_id))
    .bind(input.admin_port.unwrap_or(existing.admin_port))
    .bind(input.description.unwrap_or(existing.description))
    .bind(input.status.unwrap_or(existing.status))
    .fetch_one(&state.db)
    .await?;

    let new = serde_json::to_value(&row).unwrap_or_default();
    let mut changes = serde_json::Map::new();
    let skip = ["id", "created_at", "updated_at", "vps_id", "config_version"];
    if let (serde_json::Value::Object(old_map), serde_json::Value::Object(new_map)) = (&old, &new) {
        for (key, new_val) in new_map {
            if skip.contains(&key.as_str()) {
                continue;
            }
            if let Some(old_val) = old_map.get(key) {
                if old_val != new_val {
                    changes.insert(
                        key.clone(),
                        serde_json::json!({"from": old_val, "to": new_val}),
                    );
                }
            }
        }
    }

    log_audit(
        &state.db,
        &user,
        "update",
        "envoy_node",
        Some(&id.to_string()),
        serde_json::json!({"node_id": row.node_id, "changes": changes}),
    )
    .await;

    Ok(Json(row))
}

#[utoipa::path(
    delete,
    path = "/api/envoy-nodes/{id}",
    tag = "Envoy",
    params(("id" = Uuid, Path, description = "Envoy Node ID")),
    responses(
        (status = 200),
        (status = 404),
    )
)]
pub async fn delete_node(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    let result = sqlx::query("DELETE FROM envoy_nodes WHERE id = $1")
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
        "envoy_node",
        Some(&id.to_string()),
        serde_json::json!({}),
    )
    .await;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

// ─── Envoy Routes ────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/envoy-routes",
    tag = "Envoy",
    params(EnvoyRouteListQuery),
    responses(
        (status = 200, body = PaginatedEnvoyRouteResponse),
    )
)]
pub async fn list_routes(
    State(state): State<AppState>,
    Query(q): Query<EnvoyRouteListQuery>,
) -> Result<Json<PaginatedResponse<EnvoyRoute>>, AppError> {
    let per_page = q.per_page.clamp(1, 100);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let mut where_clause = String::from(" WHERE 1=1");
    let mut param_idx = 0u32;

    if q.envoy_node_id.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND envoy_node_id = ${param_idx}"));
    }
    if q.status.is_some() {
        param_idx += 1;
        where_clause.push_str(&format!(" AND status = ${param_idx}"));
    }

    // Count
    let count_sql = format!("SELECT COUNT(*) FROM envoy_routes{where_clause}");
    let mut count_query = sqlx::query_as::<_, (i64,)>(&count_sql);
    if let Some(ref v) = q.envoy_node_id {
        count_query = count_query.bind(v);
    }
    if let Some(ref v) = q.status {
        count_query = count_query.bind(v);
    }
    let total = count_query.fetch_one(&state.db).await?.0;

    // Data
    param_idx += 1;
    let limit_param = param_idx;
    param_idx += 1;
    let offset_param = param_idx;

    let data_sql = format!(
        "SELECT * FROM envoy_routes{where_clause} ORDER BY listen_port ASC LIMIT ${limit_param} OFFSET ${offset_param}"
    );
    let mut query = sqlx::query_as::<_, EnvoyRoute>(&data_sql);
    if let Some(ref v) = q.envoy_node_id {
        query = query.bind(v);
    }
    if let Some(ref v) = q.status {
        query = query.bind(v);
    }
    query = query.bind(per_page).bind(offset);

    let rows = query.fetch_all(&state.db).await?;

    Ok(Json(PaginatedResponse {
        data: rows,
        total,
        page,
        per_page,
    }))
}

#[utoipa::path(
    get,
    path = "/api/envoy-routes/{id}",
    tag = "Envoy",
    params(("id" = Uuid, Path, description = "Envoy Route ID")),
    responses(
        (status = 200, body = EnvoyRoute),
        (status = 404),
    )
)]
pub async fn get_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<EnvoyRoute>, AppError> {
    let row = sqlx::query_as::<_, EnvoyRoute>("SELECT * FROM envoy_routes WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(row))
}

#[utoipa::path(
    post,
    path = "/api/envoy-routes",
    tag = "Envoy",
    request_body = CreateEnvoyRoute,
    responses(
        (status = 200, body = EnvoyRoute),
    )
)]
pub async fn create_route(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Json(input): Json<CreateEnvoyRoute>,
) -> Result<Json<EnvoyRoute>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    // Validate envoy_node_id exists
    sqlx::query_as::<_, (Uuid,)>("SELECT id FROM envoy_nodes WHERE id = $1")
        .bind(input.envoy_node_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(format!("Envoy node {} not found", input.envoy_node_id))
        })?;

    let row = sqlx::query_as::<_, EnvoyRoute>(
        r#"INSERT INTO envoy_routes (envoy_node_id, name, listen_port, backend_host, backend_port, cluster_type, connect_timeout_secs, proxy_protocol, status)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
           RETURNING *"#,
    )
    .bind(input.envoy_node_id)
    .bind(&input.name)
    .bind(input.listen_port)
    .bind(&input.backend_host)
    .bind(input.backend_port)
    .bind(&input.cluster_type)
    .bind(input.connect_timeout_secs)
    .bind(input.proxy_protocol)
    .bind(&input.status)
    .fetch_one(&state.db)
    .await?;

    // Bump config_version on parent node
    sqlx::query("UPDATE envoy_nodes SET config_version = config_version + 1, updated_at = now() WHERE id = $1")
        .bind(input.envoy_node_id)
        .execute(&state.db)
        .await?;

    log_audit(
        &state.db,
        &user,
        "create",
        "envoy_route",
        Some(&row.id.to_string()),
        serde_json::json!({"name": row.name, "envoy_node_id": row.envoy_node_id.to_string(), "listen_port": row.listen_port}),
    )
    .await;

    Ok(Json(row))
}

#[utoipa::path(
    put,
    path = "/api/envoy-routes/{id}",
    tag = "Envoy",
    params(("id" = Uuid, Path, description = "Envoy Route ID")),
    request_body = UpdateEnvoyRoute,
    responses(
        (status = 200, body = EnvoyRoute),
        (status = 404),
    )
)]
pub async fn update_route(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateEnvoyRoute>,
) -> Result<Json<EnvoyRoute>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    let existing = sqlx::query_as::<_, EnvoyRoute>("SELECT * FROM envoy_routes WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let old = serde_json::to_value(&existing).unwrap_or_default();

    let row = sqlx::query_as::<_, EnvoyRoute>(
        r#"UPDATE envoy_routes SET
            name = $2, listen_port = $3, backend_host = $4, backend_port = $5,
            cluster_type = $6, connect_timeout_secs = $7, proxy_protocol = $8,
            status = $9, updated_at = now()
           WHERE id = $1
           RETURNING *"#,
    )
    .bind(id)
    .bind(input.name.unwrap_or(existing.name))
    .bind(input.listen_port.unwrap_or(existing.listen_port))
    .bind(input.backend_host.unwrap_or(existing.backend_host))
    .bind(input.backend_port.unwrap_or(existing.backend_port))
    .bind(input.cluster_type.unwrap_or(existing.cluster_type))
    .bind(input.connect_timeout_secs.unwrap_or(existing.connect_timeout_secs))
    .bind(input.proxy_protocol.unwrap_or(existing.proxy_protocol))
    .bind(input.status.unwrap_or(existing.status))
    .fetch_one(&state.db)
    .await?;

    // Bump config_version on parent node
    sqlx::query("UPDATE envoy_nodes SET config_version = config_version + 1, updated_at = now() WHERE id = $1")
        .bind(row.envoy_node_id)
        .execute(&state.db)
        .await?;

    let new = serde_json::to_value(&row).unwrap_or_default();
    let mut changes = serde_json::Map::new();
    let skip = ["id", "created_at", "updated_at", "envoy_node_id"];
    if let (serde_json::Value::Object(old_map), serde_json::Value::Object(new_map)) = (&old, &new) {
        for (key, new_val) in new_map {
            if skip.contains(&key.as_str()) {
                continue;
            }
            if let Some(old_val) = old_map.get(key) {
                if old_val != new_val {
                    changes.insert(
                        key.clone(),
                        serde_json::json!({"from": old_val, "to": new_val}),
                    );
                }
            }
        }
    }

    log_audit(
        &state.db,
        &user,
        "update",
        "envoy_route",
        Some(&id.to_string()),
        serde_json::json!({"name": row.name, "changes": changes}),
    )
    .await;

    Ok(Json(row))
}

#[utoipa::path(
    delete,
    path = "/api/envoy-routes/{id}",
    tag = "Envoy",
    params(("id" = Uuid, Path, description = "Envoy Route ID")),
    responses(
        (status = 200),
        (status = 404),
    )
)]
pub async fn delete_route(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_role(&user, &["admin", "operator"])?;

    // Fetch the route to get the parent node ID before deletion
    let route = sqlx::query_as::<_, EnvoyRoute>("SELECT * FROM envoy_routes WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    sqlx::query("DELETE FROM envoy_routes WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    // Bump config_version on parent node
    sqlx::query("UPDATE envoy_nodes SET config_version = config_version + 1, updated_at = now() WHERE id = $1")
        .bind(route.envoy_node_id)
        .execute(&state.db)
        .await?;

    log_audit(
        &state.db,
        &user,
        "delete",
        "envoy_route",
        Some(&id.to_string()),
        serde_json::json!({"name": route.name, "envoy_node_id": route.envoy_node_id.to_string()}),
    )
    .await;

    Ok(Json(serde_json::json!({ "deleted": true })))
}
