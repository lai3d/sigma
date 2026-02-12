mod common;

use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
async fn test_login_success() {
    let (router, pool) = common::setup().await;

    let body = json!({
        "email": "admin@test.local",
        "password": "testpass123"
    });

    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(json["token"].is_string());
    assert_eq!(json["user"]["email"], "admin@test.local");
    assert_eq!(json["user"]["role"], "admin");

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_login_wrong_password() {
    let (router, pool) = common::setup().await;

    let body = json!({
        "email": "admin@test.local",
        "password": "wrongpassword"
    });

    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 401);

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_login_nonexistent_user() {
    let (router, pool) = common::setup().await;

    let body = json!({
        "email": "nobody@test.local",
        "password": "anything"
    });

    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 401);

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_me_with_valid_token() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    let (status, json) = common::request_with_token(&router, "GET", "/api/auth/me", &token, None).await;

    assert_eq!(status, 200);
    assert_eq!(json["email"], "admin@test.local");
    assert_eq!(json["role"], "admin");
    assert_eq!(json["name"], "Test Admin");

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_me_without_token() {
    let (router, pool) = common::setup().await;

    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/auth/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 401);

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_me_with_api_key() {
    let (router, pool) = common::setup().await;

    let (status, json) = common::request_with_api_key(&router, "GET", "/api/auth/me", None).await;

    assert_eq!(status, 200);
    assert_eq!(json["email"], "api-key");

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_change_password() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    let body = json!({
        "current_password": "testpass123",
        "new_password": "newpass456"
    });

    let (status, _) =
        common::request_with_token(&router, "POST", "/api/auth/change-password", &token, Some(body))
            .await;

    assert_eq!(status, 200);

    // Now login with new password should work
    let login_body = json!({
        "email": "admin@test.local",
        "password": "newpass456"
    });

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&login_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_refresh_token() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    let (status, json) =
        common::request_with_token(&router, "POST", "/api/auth/refresh", &token, None).await;

    assert_eq!(status, 200);
    assert!(json["token"].is_string());
    assert_eq!(json["user"]["email"], "admin@test.local");

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_force_password_change_flag() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    // Create a user with force_password_change=true
    let body = json!({
        "email": "forced@test.local",
        "password": "temppass123",
        "name": "Forced User",
        "role": "operator"
    });

    let (status, user_json) =
        common::request_with_token(&router, "POST", "/api/users", &token, Some(body)).await;
    assert_eq!(status, 200);
    let user_id = user_json["id"].as_str().unwrap();

    // Update the user to set force_password_change
    let update_body = json!({
        "force_password_change": true
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
    assert_eq!(json["force_password_change"], true);

    // Login as that user and change password — should clear the flag
    let login_body = json!({
        "email": "forced@test.local",
        "password": "temppass123"
    });
    let login_resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&login_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login_resp.status(), 200);
    let bytes = login_resp.into_body().collect().await.unwrap().to_bytes();
    let login_json: Value = serde_json::from_slice(&bytes).unwrap();
    let user_token = login_json["token"].as_str().unwrap();

    // Change password
    let change_body = json!({
        "current_password": "temppass123",
        "new_password": "newpass789"
    });
    let (status, change_json) = common::request_with_token(
        &router,
        "POST",
        "/api/auth/change-password",
        user_token,
        Some(change_body),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(change_json["force_password_change"], false);

    common::cleanup(&pool).await;
}
