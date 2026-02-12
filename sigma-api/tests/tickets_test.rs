mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn test_create_ticket() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    let body = json!({
        "title": "Server unreachable",
        "description": "VPS in Tokyo is not responding to pings",
        "priority": "high"
    });

    let (status, json) =
        common::request_with_token(&router, "POST", "/api/tickets", &token, Some(body)).await;

    assert_eq!(status, 200);
    assert_eq!(json["title"], "Server unreachable");
    assert_eq!(json["description"], "VPS in Tokyo is not responding to pings");
    assert_eq!(json["priority"], "high");
    assert_eq!(json["status"], "open");
    assert!(json["id"].is_string());

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_list_tickets() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    // Create two tickets
    for title in &["Ticket A", "Ticket B"] {
        let body = json!({ "title": title });
        let (status, _) =
            common::request_with_token(&router, "POST", "/api/tickets", &token, Some(body)).await;
        assert_eq!(status, 200);
    }

    let (status, json) =
        common::request_with_token(&router, "GET", "/api/tickets", &token, None).await;

    assert_eq!(status, 200);
    assert_eq!(json["total"], 2);
    assert_eq!(json["data"].as_array().unwrap().len(), 2);

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_get_ticket() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    let body = json!({ "title": "Get me", "priority": "critical" });
    let (status, create_json) =
        common::request_with_token(&router, "POST", "/api/tickets", &token, Some(body)).await;
    assert_eq!(status, 200);
    let id = create_json["id"].as_str().unwrap();

    let (status, json) = common::request_with_token(
        &router,
        "GET",
        &format!("/api/tickets/{}", id),
        &token,
        None,
    )
    .await;

    assert_eq!(status, 200);
    assert_eq!(json["title"], "Get me");
    assert_eq!(json["priority"], "critical");

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_update_ticket() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    let body = json!({ "title": "Original title", "priority": "low" });
    let (status, create_json) =
        common::request_with_token(&router, "POST", "/api/tickets", &token, Some(body)).await;
    assert_eq!(status, 200);
    let id = create_json["id"].as_str().unwrap();

    let update_body = json!({
        "title": "Updated title",
        "status": "in-progress",
        "priority": "high"
    });
    let (status, json) = common::request_with_token(
        &router,
        "PUT",
        &format!("/api/tickets/{}", id),
        &token,
        Some(update_body),
    )
    .await;

    assert_eq!(status, 200);
    assert_eq!(json["title"], "Updated title");
    assert_eq!(json["status"], "in-progress");
    assert_eq!(json["priority"], "high");

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_delete_ticket() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    let body = json!({ "title": "To delete" });
    let (status, create_json) =
        common::request_with_token(&router, "POST", "/api/tickets", &token, Some(body)).await;
    assert_eq!(status, 200);
    let id = create_json["id"].as_str().unwrap();

    let (status, json) = common::request_with_token(
        &router,
        "DELETE",
        &format!("/api/tickets/{}", id),
        &token,
        None,
    )
    .await;

    assert_eq!(status, 200);
    assert_eq!(json["deleted"], true);

    // Verify deleted
    let (status, _) = common::request_with_token(
        &router,
        "GET",
        &format!("/api/tickets/{}", id),
        &token,
        None,
    )
    .await;
    assert_eq!(status, 404);

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_ticket_comments() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    // Create ticket
    let body = json!({ "title": "With comments" });
    let (status, create_json) =
        common::request_with_token(&router, "POST", "/api/tickets", &token, Some(body)).await;
    assert_eq!(status, 200);
    let id = create_json["id"].as_str().unwrap();

    // Add comment
    let comment_body = json!({ "body": "This is a test comment" });
    let (status, comment_json) = common::request_with_token(
        &router,
        "POST",
        &format!("/api/tickets/{}/comments", id),
        &token,
        Some(comment_body),
    )
    .await;

    assert_eq!(status, 200);
    assert_eq!(comment_json["body"], "This is a test comment");
    assert_eq!(comment_json["user_email"], "admin@test.local");

    // List comments
    let (status, json) = common::request_with_token(
        &router,
        "GET",
        &format!("/api/tickets/{}/comments", id),
        &token,
        None,
    )
    .await;

    assert_eq!(status, 200);
    let comments = json.as_array().unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0]["body"], "This is a test comment");

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_filter_tickets_by_status() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    // Create open ticket
    let body = json!({ "title": "Open ticket" });
    let (status, _) =
        common::request_with_token(&router, "POST", "/api/tickets", &token, Some(body)).await;
    assert_eq!(status, 200);

    // Create ticket and close it
    let body = json!({ "title": "Closed ticket" });
    let (status, create_json) =
        common::request_with_token(&router, "POST", "/api/tickets", &token, Some(body)).await;
    assert_eq!(status, 200);
    let closed_id = create_json["id"].as_str().unwrap();

    let update_body = json!({ "status": "closed" });
    let (status, _) = common::request_with_token(
        &router,
        "PUT",
        &format!("/api/tickets/{}", closed_id),
        &token,
        Some(update_body),
    )
    .await;
    assert_eq!(status, 200);

    // Filter by open
    let (status, json) = common::request_with_token(
        &router,
        "GET",
        "/api/tickets?status=open",
        &token,
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(json["total"], 1);
    assert_eq!(json["data"][0]["title"], "Open ticket");

    // Filter by closed
    let (status, json) = common::request_with_token(
        &router,
        "GET",
        "/api/tickets?status=closed",
        &token,
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(json["total"], 1);
    assert_eq!(json["data"][0]["title"], "Closed ticket");

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_filter_tickets_by_priority() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    let body = json!({ "title": "Critical issue", "priority": "critical" });
    let (status, _) =
        common::request_with_token(&router, "POST", "/api/tickets", &token, Some(body)).await;
    assert_eq!(status, 200);

    let body = json!({ "title": "Low issue", "priority": "low" });
    let (status, _) =
        common::request_with_token(&router, "POST", "/api/tickets", &token, Some(body)).await;
    assert_eq!(status, 200);

    let (status, json) = common::request_with_token(
        &router,
        "GET",
        "/api/tickets?priority=critical",
        &token,
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(json["total"], 1);
    assert_eq!(json["data"][0]["title"], "Critical issue");

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_readonly_cannot_create_ticket() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    // Create a readonly user
    let user_body = json!({
        "email": "readonly@test.local",
        "password": "password123",
        "name": "Read Only",
        "role": "readonly"
    });
    let (status, _) =
        common::request_with_token(&router, "POST", "/api/users", &token, Some(user_body)).await;
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
        .body(Body::from(serde_json::to_string(&login_body).unwrap()))
        .unwrap();
    let response = router.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), 200);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let login_json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let readonly_token = login_json["token"].as_str().unwrap();

    // Readonly can list tickets
    let (status, _) =
        common::request_with_token(&router, "GET", "/api/tickets", readonly_token, None).await;
    assert_eq!(status, 200);

    // Readonly cannot create tickets
    let body = json!({ "title": "Blocked" });
    let (status, _) =
        common::request_with_token(&router, "POST", "/api/tickets", readonly_token, Some(body))
            .await;
    assert_eq!(status, 403);

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_only_admin_can_delete_ticket() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    // Create an operator user
    let user_body = json!({
        "email": "operator@test.local",
        "password": "password123",
        "name": "Operator",
        "role": "operator"
    });
    let (status, _) =
        common::request_with_token(&router, "POST", "/api/users", &token, Some(user_body)).await;
    assert_eq!(status, 200);

    // Login as operator
    let login_body = json!({
        "email": "operator@test.local",
        "password": "password123"
    });
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&login_body).unwrap()))
        .unwrap();
    let response = router.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), 200);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let login_json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let operator_token = login_json["token"].as_str().unwrap();

    // Operator can create ticket
    let body = json!({ "title": "Operator ticket" });
    let (status, create_json) =
        common::request_with_token(&router, "POST", "/api/tickets", operator_token, Some(body))
            .await;
    assert_eq!(status, 200);
    let id = create_json["id"].as_str().unwrap();

    // Operator cannot delete ticket
    let (status, _) = common::request_with_token(
        &router,
        "DELETE",
        &format!("/api/tickets/{}", id),
        operator_token,
        None,
    )
    .await;
    assert_eq!(status, 403);

    // Admin can delete ticket
    let (status, _) = common::request_with_token(
        &router,
        "DELETE",
        &format!("/api/tickets/{}", id),
        &token,
        None,
    )
    .await;
    assert_eq!(status, 200);

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_ticket_status_transitions() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    let body = json!({ "title": "Status flow test" });
    let (status, create_json) =
        common::request_with_token(&router, "POST", "/api/tickets", &token, Some(body)).await;
    assert_eq!(status, 200);
    let id = create_json["id"].as_str().unwrap();
    assert_eq!(create_json["status"], "open");

    // open -> in-progress
    let (status, json) = common::request_with_token(
        &router,
        "PUT",
        &format!("/api/tickets/{}", id),
        &token,
        Some(json!({ "status": "in-progress" })),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(json["status"], "in-progress");

    // in-progress -> resolved
    let (status, json) = common::request_with_token(
        &router,
        "PUT",
        &format!("/api/tickets/{}", id),
        &token,
        Some(json!({ "status": "resolved" })),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(json["status"], "resolved");

    // resolved -> closed
    let (status, json) = common::request_with_token(
        &router,
        "PUT",
        &format!("/api/tickets/{}", id),
        &token,
        Some(json!({ "status": "closed" })),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(json["status"], "closed");

    // closed -> open (reopen)
    let (status, json) = common::request_with_token(
        &router,
        "PUT",
        &format!("/api/tickets/{}", id),
        &token,
        Some(json!({ "status": "open" })),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(json["status"], "open");

    common::cleanup(&pool).await;
}
