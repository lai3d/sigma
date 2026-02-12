mod common;

use axum::body::Body;
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn test_create_provider() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    let body = json!({
        "name": "TestProvider",
        "country": "US",
        "website": "https://test.example.com",
        "api_supported": true,
        "rating": 5,
        "notes": "Test provider"
    });

    let (status, json) =
        common::request_with_token(&router, "POST", "/api/providers", &token, Some(body)).await;

    assert_eq!(status, 200);
    assert_eq!(json["name"], "TestProvider");
    assert_eq!(json["country"], "US");
    assert_eq!(json["api_supported"], true);
    assert_eq!(json["rating"], 5);
    assert!(json["id"].is_string());

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_list_providers() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    // Create two providers
    for name in &["ProviderA", "ProviderB"] {
        let body = json!({ "name": name, "country": "DE" });
        let (status, _) =
            common::request_with_token(&router, "POST", "/api/providers", &token, Some(body))
                .await;
        assert_eq!(status, 200);
    }

    let (status, json) =
        common::request_with_token(&router, "GET", "/api/providers", &token, None).await;

    assert_eq!(status, 200);
    assert_eq!(json["total"], 2);
    assert_eq!(json["data"].as_array().unwrap().len(), 2);

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_update_provider() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    // Create
    let body = json!({ "name": "OldName", "country": "JP" });
    let (status, create_json) =
        common::request_with_token(&router, "POST", "/api/providers", &token, Some(body)).await;
    assert_eq!(status, 200);
    let id = create_json["id"].as_str().unwrap();

    // Update
    let update_body = json!({ "name": "NewName", "country": "KR" });
    let (status, json) = common::request_with_token(
        &router,
        "PUT",
        &format!("/api/providers/{}", id),
        &token,
        Some(update_body),
    )
    .await;

    assert_eq!(status, 200);
    assert_eq!(json["name"], "NewName");
    assert_eq!(json["country"], "KR");

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_delete_provider() {
    let (router, pool) = common::setup().await;
    let token = common::login_admin(&router).await;

    // Create
    let body = json!({ "name": "ToDelete", "country": "US" });
    let (status, create_json) =
        common::request_with_token(&router, "POST", "/api/providers", &token, Some(body)).await;
    assert_eq!(status, 200);
    let id = create_json["id"].as_str().unwrap();

    // Delete
    let (status, json) = common::request_with_token(
        &router,
        "DELETE",
        &format!("/api/providers/{}", id),
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
        &format!("/api/providers/{}", id),
        &token,
        None,
    )
    .await;
    assert_eq!(status, 404);

    common::cleanup(&pool).await;
}

#[tokio::test]
async fn test_readonly_cannot_create_provider() {
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

    // Try to create a provider as readonly
    let body = json!({ "name": "Blocked", "country": "US" });
    let (status, _) =
        common::request_with_token(&router, "POST", "/api/providers", readonly_token, Some(body))
            .await;
    assert_eq!(status, 403);

    common::cleanup(&pool).await;
}
