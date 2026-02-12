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
async fn preview_from_url_uses_mock_api_base() {
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
        "/api/preview",
        Method::POST,
        json!({
            "quack_input": "https://purrly.ai/discovery/share/testsid",
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null
        }),
    );

    let res = timeout(Duration::from_secs(5), app.oneshot(req))
        .await
        .expect("request timed out")
        .expect("request failed");

    let v = common::http::read_json_response(res).await;
    assert_eq!(v["success"], true);
    assert_eq!(v["data"]["name"], "Test Character");
    assert_eq!(v["data"]["source"], "api");
}

#[tokio::test]
async fn preview_from_manual_json_does_not_need_network() {
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
        "/api/preview",
        Method::POST,
        json!({
            "quack_input": manual,
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null
        }),
    );

    let res = timeout(Duration::from_secs(5), app.oneshot(req))
        .await
        .expect("request timed out")
        .expect("request failed");

    let v = common::http::read_json_response(res).await;
    assert_eq!(v["success"], true);
    assert_eq!(v["data"]["name"], "Manual Character");
    assert_eq!(v["data"]["source"], "json");
}

#[tokio::test]
async fn preview_invalid_url_maps_error_code() {
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
        "/api/preview",
        Method::POST,
        json!({
            "quack_input": "https://purrly.ai/discovery/share/",
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null
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
async fn preview_unauthorized_maps_error_code() {
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
        "/api/preview",
        Method::POST,
        json!({
            "quack_input": "https://purrly.ai/discovery/share/unauthorized",
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null
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
async fn preview_invalid_bearer_token_maps_parse_error() {
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
        "/api/preview",
        Method::POST,
        json!({
            "quack_input": "https://purrly.ai/discovery/share/testsid",
            "cookies": null,
            "bearer_token": "bad\nvalue",
            "user_agent": null,
            "email": null,
            "password": null
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
