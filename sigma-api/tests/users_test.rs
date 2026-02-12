mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn test_create_user() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    let body = json!({
        "email": "newuser@test.local",
        "password": "password123",
        "name": "New User",
        "role": "operator"
    });

    let (status, json) =
        common::request_with_token(&router, "POST", "/api/users", &token, Some(body)).await;

    assert_eq!(status, 200);
    assert_eq!(json["email"], "newuser@test.local");
    assert_eq!(json["name"], "New User");
    assert_eq!(json["role"], "operator");
    assert!(json["id"].is_string());

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_list_users() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    let (status, json) =
        common::request_with_token(&router, "GET", "/api/users", &token, None).await;

    assert_eq!(status, 200);
    assert_eq!(json["total"], 1); // Just the seeded admin
    assert_eq!(json["page"], 1);
    assert!(json["data"].is_array());
    assert_eq!(json["data"].as_array().unwrap().len(), 1);

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_update_user_role() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    // Create a user first
    let create_body = json!({
        "email": "update-me@test.local",
        "password": "password123",
        "name": "Update Me",
        "role": "readonly"
    });
    let (status, user_json) =
        common::request_with_token(&router, "POST", "/api/users", &token, Some(create_body)).await;
    assert_eq!(status, 200);
    let user_id = user_json["id"].as_str().unwrap();

    // Update role
    let update_body = json!({
        "role": "operator"
    });
    let (status, json) = common::request_with_token(
        &router,
        "PUT",
        &format!("/api/users/{}", user_id),
        &token,
        Some(update_body),
    )
    .await;

    assert_eq!(status, 200);
    assert_eq!(json["role"], "operator");

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_delete_user() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    // Create a user to delete
    let create_body = json!({
        "email": "delete-me@test.local",
        "password": "password123",
        "name": "Delete Me",
        "role": "readonly"
    });
    let (status, user_json) =
        common::request_with_token(&router, "POST", "/api/users", &token, Some(create_body)).await;
    assert_eq!(status, 200);
    let user_id = user_json["id"].as_str().unwrap();

    // Delete the user
    let (status, json) = common::request_with_token(
        &router,
        "DELETE",
        &format!("/api/users/{}", user_id),
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
        &format!("/api/users/{}", user_id),
        &token,
        None,
    )
    .await;
    assert_eq!(status, 404);

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_readonly_cannot_create_user() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    // Create a readonly user
    let create_body = json!({
        "email": "readonly@test.local",
        "password": "password123",
        "name": "Read Only",
        "role": "readonly"
    });
    let (status, _) =
        common::request_with_token(&router, "POST", "/api/users", &token, Some(create_body)).await;
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

    // Try to create a user as readonly — should be forbidden
    let body = json!({
        "email": "another@test.local",
        "password": "password123",
        "name": "Another",
        "role": "readonly"
    });
    let (status, _) =
        common::request_with_token(&router, "POST", "/api/users", readonly_token, Some(body))
            .await;
    assert_eq!(status, 403);

    common::cleanup(&pool).await;
}
