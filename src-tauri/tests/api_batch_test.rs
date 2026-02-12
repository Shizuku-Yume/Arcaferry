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
async fn batch_mixed_results_are_reported() {
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
        "/api/batch",
        Method::POST,
        json!({
            "urls": [
                "https://purrly.ai/discovery/share/testsid",
                "https://purrly.ai/discovery/share/unauthorized"
            ],
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null,
            "gemini_api_key": null,
            "concurrency": 2,
            "output_format": "json"
        }),
    );

    let res = timeout(Duration::from_secs(10), app.oneshot(req))
        .await
        .expect("request timed out")
        .expect("request failed");

    let v = common::http::read_json_response(res).await;
    assert_eq!(v["total"], 2);
    assert_eq!(v["succeeded"], 1);
    assert_eq!(v["failed"], 1);
    assert_eq!(v["success"], false);

    let results = v["results"].as_array().expect("results should be array");
    assert_eq!(results.len(), 2);
    let unauthorized = results
        .iter()
        .find(|r| r["url"].as_str().unwrap_or("").contains("unauthorized"))
        .expect("missing unauthorized result");
    assert_eq!(unauthorized["success"], false);
    assert_eq!(unauthorized["error_code"], "UNAUTHORIZED");
}

#[tokio::test]
async fn batch_empty_urls_maps_error_code() {
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
        "/api/batch",
        Method::POST,
        json!({
            "urls": [],
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null,
            "gemini_api_key": null,
            "concurrency": 1,
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
async fn batch_invalid_bearer_token_maps_parse_error() {
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
        "/api/batch",
        Method::POST,
        json!({
            "urls": ["https://purrly.ai/discovery/share/testsid"],
            "cookies": null,
            "bearer_token": "bad\nvalue",
            "user_agent": null,
            "email": null,
            "password": null,
            "gemini_api_key": null,
            "concurrency": 1,
            "output_format": "json"
        }),
    );

    let res = timeout(Duration::from_secs(10), app.oneshot(req))
        .await
        .expect("request timed out")
        .expect("request failed");

    // Batch endpoint reports per-item failures while returning 200.
    let v = common::http::read_json_response(res).await;
    assert_eq!(v["total"], 1);
    assert_eq!(v["success"], false);
    let results = v["results"].as_array().expect("results should be array");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["success"], false);
    assert_eq!(results[0]["error_code"], "PARSE_ERROR");
}

#[tokio::test]
async fn batch_output_format_png_returns_png_base64() {
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
        "/api/batch",
        Method::POST,
        json!({
            "urls": [
                "https://purrly.ai/discovery/share/testsid"
            ],
            "cookies": null,
            "bearer_token": null,
            "user_agent": null,
            "email": null,
            "password": null,
            "gemini_api_key": null,
            "concurrency": 1,
            "output_format": "png"
        }),
    );

    let res = timeout(Duration::from_secs(10), app.oneshot(req))
        .await
        .expect("request timed out")
        .expect("request failed");

    let v = common::http::read_json_response(res).await;
    assert_eq!(v["success"], true);
    assert_eq!(v["total"], 1);
    assert_eq!(v["failed"], 0);
    let results = v["results"].as_array().expect("results should be array");
    assert_eq!(results.len(), 1);
    assert!(results[0]["png_base64"].as_str().is_some());
    assert!(results[0]["avatar_base64"].is_null());
}
