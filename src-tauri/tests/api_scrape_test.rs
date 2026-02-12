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
async fn scrape_happy_path_is_hermetic() {
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
        "/api/scrape",
        Method::POST,
        json!({
            "url": "https://purrly.ai/discovery/share/testsid",
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null,
            "gemini_api_key": null,
            "output_format": "json"
        }),
    );

    let res = timeout(Duration::from_secs(5), app.oneshot(req))
        .await
        .expect("request timed out")
        .expect("request failed");

    let v = common::http::read_json_response(res).await;
    assert_eq!(v["success"], true);
    assert_eq!(v["card"]["spec"], "chara_card_v3");
    assert_eq!(v["card"]["data"]["name"], "Test Character");
    assert!(v["warnings"].as_array().is_some());
}

#[tokio::test]
async fn scrape_unauthorized_maps_error_code() {
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
        "/api/scrape",
        Method::POST,
        json!({
            "url": "https://purrly.ai/discovery/share/unauthorized",
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null,
            "gemini_api_key": null,
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
async fn scrape_invalid_url_maps_error_code() {
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
        "/api/scrape",
        Method::POST,
        json!({
            "url": "https://purrly.ai/discovery/share/",
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null,
            "gemini_api_key": null,
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
async fn scrape_rate_limited_maps_error_code() {
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
        "/api/scrape",
        Method::POST,
        json!({
            "url": "https://purrly.ai/discovery/share/rate_limited",
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null,
            "gemini_api_key": null,
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
async fn scrape_cloudflare_blocked_attaches_guidance() {
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
        "/api/scrape",
        Method::POST,
        json!({
            "url": "https://purrly.ai/discovery/share/cloudflare",
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null,
            "gemini_api_key": null,
            "output_format": "json"
        }),
    );

    let res = timeout(Duration::from_secs(5), app.oneshot(req))
        .await
        .expect("request timed out")
        .expect("request failed");

    assert_eq!(res.status(), axum::http::StatusCode::SERVICE_UNAVAILABLE);
    let v = common::http::read_json_response(res).await;
    assert_eq!(v["success"], false);
    assert_eq!(v["error_code"], "CLOUDFLARE_BLOCKED");
    let err = v["error"].as_str().unwrap_or("");
    assert!(err.contains("cf_clearance"), "error should mention cf_clearance guidance");
}

#[tokio::test]
async fn scrape_invalid_json_maps_parse_error() {
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
        "/api/scrape",
        Method::POST,
        json!({
            "url": "https://purrly.ai/discovery/share/badjson",
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null,
            "gemini_api_key": null,
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

#[tokio::test]
async fn scrape_output_format_png_returns_png_base64() {
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
        "/api/scrape",
        Method::POST,
        json!({
            "url": "https://purrly.ai/discovery/share/testsid",
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null,
            "gemini_api_key": null,
            "output_format": "png"
        }),
    );

    let res = timeout(Duration::from_secs(5), app.oneshot(req))
        .await
        .expect("request timed out")
        .expect("request failed");

    let v = common::http::read_json_response(res).await;
    assert_eq!(v["success"], true);
    assert!(v["png_base64"].as_str().is_some());
    // For PNG output we intentionally omit avatar_base64 from the response.
    assert!(v["avatar_base64"].is_null());
}

#[tokio::test]
async fn scrape_timeout_maps_error_code() {
    let mock = common::mock_quack::MockQuackServer::start().await;

    let state = Arc::new(ServerState {
        port: 0,
        browser_capability: BrowserCapability::NotInstalled {
            reason: "test".to_string(),
        },
        api_base_override: Some(mock.base_url.clone()),
        http_timeout_secs_override: Some(1),
    });
    let app = create_router(state);

    let req = common::http::json_request(
        "/api/scrape",
        Method::POST,
        json!({
            "url": "https://purrly.ai/discovery/share/timeout",
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null,
            "gemini_api_key": null,
            "output_format": "json"
        }),
    );

    let res = timeout(Duration::from_secs(5), app.oneshot(req))
        .await
        .expect("request timed out")
        .expect("request failed");

    assert_eq!(res.status(), axum::http::StatusCode::GATEWAY_TIMEOUT);
    let v = common::http::read_json_response(res).await;
    assert_eq!(v["success"], false);
    assert_eq!(v["error_code"], "TIMEOUT");
}

#[tokio::test]
async fn scrape_avatar_fetch_failure_is_nonfatal() {
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
        "/api/scrape",
        Method::POST,
        json!({
            "url": "https://purrly.ai/discovery/share/noavatar",
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null,
            "gemini_api_key": null,
            "output_format": "json"
        }),
    );

    let res = timeout(Duration::from_secs(5), app.oneshot(req))
        .await
        .expect("request timed out")
        .expect("request failed");

    let v = common::http::read_json_response(res).await;
    assert_eq!(v["success"], true);
    assert!(v["avatar_base64"].is_null(), "avatar should be omitted on failure");
    assert_eq!(v["card"]["data"]["name"], "Test Character");
}

#[tokio::test]
async fn scrape_invalid_bearer_token_maps_parse_error() {
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
        "/api/scrape",
        Method::POST,
        json!({
            "url": "https://purrly.ai/discovery/share/testsid",
            "cookies": null,
            "bearer_token": "bad\nvalue",
            "user_agent": null,
            "email": null,
            "password": null,
            "gemini_api_key": null,
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
