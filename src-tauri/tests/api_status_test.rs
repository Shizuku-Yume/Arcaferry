mod common;

use axum::http::{Request, StatusCode};
use arcaferry_lib::{
    browser_sidecar::BrowserCapability,
    server::{create_router, ServerState},
};
use std::{sync::Arc, time::Duration};
use tokio::time::timeout;
use tower::ServiceExt;

#[tokio::test]
async fn status_returns_ok_and_sidecar_fields() {
    let state = Arc::new(ServerState {
        port: 0,
        browser_capability: BrowserCapability::NotInstalled {
            reason: "test: sidecar not installed".to_string(),
        },
        api_base_override: None,
        http_timeout_secs_override: None,
    });

    let app = create_router(state);

    let req = Request::builder()
        .uri("/api/status")
        .method("GET")
        .body(axum::body::Body::empty())
        .unwrap();

    let res = timeout(Duration::from_secs(3), app.oneshot(req))
        .await
        .expect("request timed out")
        .expect("request failed");

    assert_eq!(res.status(), StatusCode::OK);
    let v = common::http::read_json_response(res).await;
    assert_eq!(v["status"], "ok");
    assert_eq!(v["ready"], true);
    assert_eq!(v["browser_extraction_available"], false);
    assert_eq!(v["browser_extraction_reason"], "test: sidecar not installed");
    assert!(v.get("supported_browsers").is_some());
}
