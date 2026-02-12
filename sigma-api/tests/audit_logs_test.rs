mod common;

use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn test_audit_logs_empty() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    // Login itself generates an audit entry, but let's verify the endpoint works
    let (status, json) =
        common::request_with_token(&router, "GET", "/api/audit-logs", &token, None).await;

    assert_eq!(status, 200);
    assert!(json["data"].is_array());
    assert!(json["total"].is_number());
    assert_eq!(json["page"], 1);

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_audit_logs_after_login() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    // Login should have created an audit entry
    let (status, json) =
        common::request_with_token(&router, "GET", "/api/audit-logs?action=login", &token, None)
            .await;

    assert_eq!(status, 200);
    let entries = json["data"].as_array().unwrap();
    assert!(!entries.is_empty(), "Login should produce an audit entry");

    let entry = &entries[0];
    assert_eq!(entry["action"], "login");
    assert_eq!(entry["resource"], "user");
    assert_eq!(entry["user_email"], "admin@test.local");

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_audit_logs_provider_crud() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    // Create provider
    let body = json!({ "name": "AuditTestProvider", "country": "US" });
    let (status, create_json) =
        common::request_with_token(&router, "POST", "/api/providers", &token, Some(body)).await;
    assert_eq!(status, 200);
    let provider_id = create_json["id"].as_str().unwrap();

    // Update provider
    let update_body = json!({ "name": "AuditTestUpdated" });
    let (status, _) = common::request_with_token(
        &router,
        "PUT",
        &format!("/api/providers/{}", provider_id),
        &token,
        Some(update_body),
    )
    .await;
    assert_eq!(status, 200);

    // Delete provider
    let (status, _) = common::request_with_token(
        &router,
        "DELETE",
        &format!("/api/providers/{}", provider_id),
        &token,
        None,
    )
    .await;
    assert_eq!(status, 200);

    // Check audit logs filtered by resource=provider
    let (status, json) = common::request_with_token(
        &router,
        "GET",
        "/api/audit-logs?resource=provider",
        &token,
        None,
    )
    .await;

    assert_eq!(status, 200);
    let entries = json["data"].as_array().unwrap();
    assert_eq!(entries.len(), 3, "Should have create, update, delete entries");

    // Entries are ordered by created_at DESC, so: delete, update, create
    assert_eq!(entries[0]["action"], "delete");
    assert_eq!(entries[1]["action"], "update");
    assert_eq!(entries[2]["action"], "create");

    // All should reference the same resource_id
    for entry in entries {
        assert_eq!(entry["resource_id"], provider_id);
        assert_eq!(entry["resource"], "provider");
    }

    // Create entry should have the name in details
    assert_eq!(entries[2]["details"]["name"], "AuditTestProvider");
    // Update entry should have the updated name
    assert_eq!(entries[1]["details"]["name"], "AuditTestUpdated");

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_audit_logs_vps_create_and_retire() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    // Create a VPS
    let body = json!({
        "hostname": "audit-test-vps",
        "country": "JP",
        "ip_addresses": [{"ip": "1.2.3.4", "label": "overseas"}],
        "status": "active"
    });
    let (status, vps_json) =
        common::request_with_token(&router, "POST", "/api/vps", &token, Some(body)).await;
    assert_eq!(status, 200);
    let vps_id = vps_json["id"].as_str().unwrap();

    // Retire the VPS
    let (status, _) = common::request_with_token(
        &router,
        "POST",
        &format!("/api/vps/{}/retire", vps_id),
        &token,
        None,
    )
    .await;
    assert_eq!(status, 200);

    // Check audit logs for this VPS
    let (status, json) = common::request_with_token(
        &router,
        "GET",
        &format!("/api/audit-logs?resource=vps&resource_id={}", vps_id),
        &token,
        None,
    )
    .await;

    assert_eq!(status, 200);
    let entries = json["data"].as_array().unwrap();
    assert_eq!(entries.len(), 2, "Should have create and retire entries");

    // DESC order: retire, create
    assert_eq!(entries[0]["action"], "retire");
    assert_eq!(entries[1]["action"], "create");
    assert_eq!(entries[1]["details"]["hostname"], "audit-test-vps");
    assert_eq!(entries[1]["details"]["country"], "JP");

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_audit_logs_user_crud() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    // Create user
    let body = json!({
        "email": "audituser@test.local",
        "password": "password123",
        "name": "Audit User",
        "role": "operator"
    });
    let (status, user_json) =
        common::request_with_token(&router, "POST", "/api/users", &token, Some(body)).await;
    assert_eq!(status, 200);
    let user_id = user_json["id"].as_str().unwrap();

    // Delete user
    let (status, _) = common::request_with_token(
        &router,
        "DELETE",
        &format!("/api/users/{}", user_id),
        &token,
        None,
    )
    .await;
    assert_eq!(status, 200);

    // Check audit logs for user resource
    let (status, json) = common::request_with_token(
        &router,
        "GET",
        "/api/audit-logs?resource=user&action=create",
        &token,
        None,
    )
    .await;

    assert_eq!(status, 200);
    let entries = json["data"].as_array().unwrap();
    // Find the entry for our created user
    let create_entry = entries
        .iter()
        .find(|e| e["resource_id"].as_str() == Some(user_id))
        .expect("Should find create entry for user");
    assert_eq!(create_entry["details"]["email"], "audituser@test.local");
    assert_eq!(create_entry["details"]["role"], "operator");

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_audit_logs_requires_admin() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    // Create a readonly user
    let body = json!({
        "email": "readonly@test.local",
        "password": "password123",
        "name": "Read Only",
        "role": "readonly"
    });
    let (status, _) =
        common::request_with_token(&router, "POST", "/api/users", &token, Some(body)).await;
    assert_eq!(status, 200);

    // Login as readonly
    let login_body = json!({
        "email": "readonly@test.local",
        "password": "password123"
    });
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            serde_json::to_string(&login_body).unwrap(),
        ))
        .unwrap();
    let response: axum::response::Response = router.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), 200);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let login_json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let readonly_token = login_json["token"].as_str().unwrap();

    // Try to access audit logs as readonly — should be forbidden
    let (status, _) =
        common::request_with_token(&router, "GET", "/api/audit-logs", readonly_token, None).await;
    assert_eq!(status, 403);

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_audit_logs_pagination() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    // Create several providers to generate audit entries
    for i in 0..5 {
        let body = json!({ "name": format!("PaginationTest{}", i), "country": "US" });
        let (status, _) =
            common::request_with_token(&router, "POST", "/api/providers", &token, Some(body))
                .await;
        assert_eq!(status, 200);
    }

    // Fetch with small page size
    let (status, json) = common::request_with_token(
        &router,
        "GET",
        "/api/audit-logs?resource=provider&per_page=2&page=1",
        &token,
        None,
    )
    .await;

    assert_eq!(status, 200);
    assert_eq!(json["data"].as_array().unwrap().len(), 2);
    assert_eq!(json["total"], 5);
    assert_eq!(json["page"], 1);
    assert_eq!(json["per_page"], 2);

    // Page 2
    let (status, json) = common::request_with_token(
        &router,
        "GET",
        "/api/audit-logs?resource=provider&per_page=2&page=2",
        &token,
        None,
    )
    .await;

    assert_eq!(status, 200);
    assert_eq!(json["data"].as_array().unwrap().len(), 2);
    assert_eq!(json["page"], 2);

    common::cleanup(&pool).await;
}
