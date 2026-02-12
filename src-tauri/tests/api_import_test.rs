mod common;

use axum::http::Method;
use arcaferry_lib::{
    browser_sidecar::BrowserCapability,
    server::{create_router, ServerState},
};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::time::timeout;
use tower::ServiceExt;

#[tokio::test]
async fn import_from_url_uses_mock_api_base() {
    let mock = common::mock_quack::MockQuackServer::start().await;

    let state = Arc::new(ServerState {
        port: 0,
        browser_capability: BrowserCapability::NotInstalled {
            reason: "test".to_string(),
        },
        api_base_override: Some(mock.base_url.clone()),
        http_timeout_secs_override: None,
    });
    let app = create_router(state);

    let req = common::http::json_request(
        "/api/import",
        Method::POST,
        json!({
            "quack_input": "https://purrly.ai/discovery/share/testsid",
            "lorebook_json": null,
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null,
            "gemini_api_key": null,
            "mode": "full",
            "output_format": "json"
        }),
    );

    let res = timeout(Duration::from_secs(5), app.oneshot(req))
        .await
        .expect("request timed out")
        .expect("request failed");

    let v = common::http::read_json_response(res).await;
    assert_eq!(v["success"], true);
    assert_eq!(v["source"], "api");
    assert_eq!(v["card"]["data"]["name"], "Test Character");
}

#[tokio::test]
async fn import_from_manual_json_does_not_need_network() {
    let state = Arc::new(ServerState {
        port: 0,
        browser_capability: BrowserCapability::NotInstalled {
            reason: "test".to_string(),
        },
        api_base_override: None,
        http_timeout_secs_override: None,
    });
    let app = create_router(state);

    let manual = serde_json::to_string(&json!({"name": "Manual Character"}))
        .expect("failed to build manual json");

    let req = common::http::json_request(
        "/api/import",
        Method::POST,
        json!({
            "quack_input": manual,
            "lorebook_json": null,
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null,
            "gemini_api_key": null,
            "mode": "full",
            "output_format": "json"
        }),
    );

    let res = timeout(Duration::from_secs(5), app.oneshot(req))
        .await
        .expect("request timed out")
        .expect("request failed");

    let v = common::http::read_json_response(res).await;
    assert_eq!(v["success"], true);
    assert_eq!(v["source"], "json");
    assert_eq!(v["card"]["data"]["name"], "Manual Character");
}

#[tokio::test]
async fn import_invalid_url_maps_error_code() {
    let mock = common::mock_quack::MockQuackServer::start().await;

    let state = Arc::new(ServerState {
        port: 0,
        browser_capability: BrowserCapability::NotInstalled {
            reason: "test".to_string(),
        },
        api_base_override: Some(mock.base_url.clone()),
        http_timeout_secs_override: None,
    });
    let app = create_router(state);

    let req = common::http::json_request(
        "/api/import",
        Method::POST,
        json!({
            "quack_input": "https://purrly.ai/discovery/share/",
            "lorebook_json": null,
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null,
            "gemini_api_key": null,
            "mode": "full",
            "output_format": "json"
        }),
    );

    let res = timeout(Duration::from_secs(5), app.oneshot(req))
        .await
        .expect("request timed out")
        .expect("request failed");

    assert_eq!(res.status(), axum::http::StatusCode::BAD_REQUEST);
    let v = common::http::read_json_response(res).await;
    assert_eq!(v["success"], false);
    assert_eq!(v["error_code"], "INVALID_URL");
}

#[tokio::test]
async fn import_unauthorized_maps_error_code() {
    let mock = common::mock_quack::MockQuackServer::start().await;

    let state = Arc::new(ServerState {
        port: 0,
        browser_capability: BrowserCapability::NotInstalled {
            reason: "test".to_string(),
        },
        api_base_override: Some(mock.base_url.clone()),
        http_timeout_secs_override: None,
    });
    let app = create_router(state);

    let req = common::http::json_request(
        "/api/import",
        Method::POST,
        json!({
            "quack_input": "https://purrly.ai/discovery/share/unauthorized",
            "lorebook_json": null,
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null,
            "gemini_api_key": null,
            "mode": "full",
            "output_format": "json"
        }),
    );

    let res = timeout(Duration::from_secs(5), app.oneshot(req))
        .await
        .expect("request timed out")
        .expect("request failed");

    assert_eq!(res.status(), axum::http::StatusCode::UNAUTHORIZED);
    let v = common::http::read_json_response(res).await;
    assert_eq!(v["success"], false);
    assert_eq!(v["error_code"], "UNAUTHORIZED");
}

#[tokio::test]
async fn import_rate_limited_maps_error_code() {
    let mock = common::mock_quack::MockQuackServer::start().await;

    let state = Arc::new(ServerState {
        port: 0,
        browser_capability: BrowserCapability::NotInstalled {
            reason: "test".to_string(),
        },
        api_base_override: Some(mock.base_url.clone()),
        http_timeout_secs_override: None,
    });
    let app = create_router(state);

    let req = common::http::json_request(
        "/api/import",
        Method::POST,
        json!({
            "quack_input": "https://purrly.ai/discovery/share/rate_limited",
            "lorebook_json": null,
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null,
            "gemini_api_key": null,
            "mode": "full",
            "output_format": "json"
        }),
    );

    let res = timeout(Duration::from_secs(5), app.oneshot(req))
        .await
        .expect("request timed out")
        .expect("request failed");

    assert_eq!(res.status(), axum::http::StatusCode::TOO_MANY_REQUESTS);
    let v = common::http::read_json_response(res).await;
    assert_eq!(v["success"], false);
    assert_eq!(v["error_code"], "RATE_LIMITED");
}

#[tokio::test]
async fn import_output_format_png_returns_png_base64() {
    let state = Arc::new(ServerState {
        port: 0,
        browser_capability: BrowserCapability::NotInstalled {
            reason: "test".to_string(),
        },
        api_base_override: None,
        http_timeout_secs_override: None,
    });
    let app = create_router(state);

    let manual = serde_json::to_string(&json!({"name": "Manual Character"}))
        .expect("failed to build manual json");

    let req = common::http::json_request(
        "/api/import",
        Method::POST,
        json!({
            "quack_input": manual,
            "lorebook_json": null,
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null,
            "gemini_api_key": null,
            "mode": "full",
            "output_format": "png"
        }),
    );

    let res = timeout(Duration::from_secs(5), app.oneshot(req))
        .await
        .expect("request timed out")
        .expect("request failed");

    let v = common::http::read_json_response(res).await;
    assert_eq!(v["success"], true);
    assert_eq!(v["source"], "json");
    assert!(v["png_base64"].as_str().is_some());
    assert!(v["avatar_base64"].is_null());
}

#[tokio::test]
async fn import_invalid_bearer_token_maps_parse_error() {
    let state = Arc::new(ServerState {
        port: 0,
        browser_capability: BrowserCapability::NotInstalled {
            reason: "test".to_string(),
        },
        api_base_override: None,
        http_timeout_secs_override: None,
    });
    let app = create_router(state);

    let req = common::http::json_request(
        "/api/import",
        Method::POST,
        json!({
            "quack_input": "https://purrly.ai/discovery/share/testsid",
            "lorebook_json": null,
            "cookies": null,
            "bearer_token": "bad\nvalue",
            "user_agent": null,
            "email": null,
            "password": null,
            "gemini_api_key": null,
            "mode": "full",
            "output_format": "json"
        }),
    );

    let res = timeout(Duration::from_secs(5), app.oneshot(req))
        .await
        .expect("request timed out")
        .expect("request failed");

    assert_eq!(res.status(), axum::http::StatusCode::BAD_REQUEST);
    let v = common::http::read_json_response(res).await;
    assert_eq!(v["success"], false);
    assert_eq!(v["error_code"], "PARSE_ERROR");
}
