#![allow(dead_code)]

use axum::{
    body::Bytes,
    extract::Query,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde_json::json;
use std::{collections::HashMap, net::SocketAddr};
use tokio::{net::TcpListener, task::JoinHandle};

/// Minimal local mock server for Quack/Purrly API + avatar download.
///
/// This is used by integration tests to ensure *no external network* is needed.
pub struct MockQuackServer {
    pub base_url: String,
    _task: JoinHandle<()>,
}

impl MockQuackServer {
    pub async fn start() -> Self {
        async fn avatar_png() -> impl IntoResponse {
            // 1x1 transparent PNG
            const PNG: &[u8] = &[
                0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
                0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
                0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00,
                0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
                0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
                0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
            ];
            let mut headers = HeaderMap::new();
            headers.insert(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static("image/png"),
            );
            (headers, Bytes::from_static(PNG))
        }

        // Build router. We'll rewrite the __MOCK_BASE__ placeholder in a middleware-like handler
        // by capturing base_url after binding.
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind mock server");
        let addr: SocketAddr = listener
            .local_addr()
            .expect("failed to get mock server addr");
        let base_url = format!("http://{}", addr);

        let base_url_for_handler = base_url.clone();
        let app = Router::new()
            .route(
                "/api/v1/studioCard/info",
                get(move |Query(q): Query<HashMap<String, String>>| {
                    let base_url = base_url_for_handler.clone();
                    async move {
                        // QuackClient (guest) calls:
                        //   /api/v1/studioCard/info?isguest=1&sid=<sid>
                        let sid = q.get("sid").map(|s| s.as_str()).unwrap_or("");

                        match sid {
                            "unauthorized" => {
                                return (StatusCode::UNAUTHORIZED, "missing token")
                                    .into_response();
                            }
                            "rate_limited" => {
                                return (StatusCode::TOO_MANY_REQUESTS, "slow down")
                                    .into_response();
                            }
                            "cloudflare" => {
                                // Simulate Cloudflare challenge page.
                                let html = "<!DOCTYPE html><title>Just a moment...</title>cloudflare";
                                return (StatusCode::FORBIDDEN, html).into_response();
                            }
                            "html" => {
                                // 200 OK but HTML body should still be detected.
                                let html = "<!DOCTYPE html><title>Just a moment...</title>cloudflare";
                                return (StatusCode::OK, html).into_response();
                            }
                            "badjson" => {
                                // 200 OK but invalid JSON.
                                return (StatusCode::OK, "{not-json")
                                    .into_response();
                            }
                            "timeout" => {
                                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            }
                            _ => {}
                        }

                        let picture = if sid == "noavatar" {
                            format!("{}/missing.png", base_url)
                        } else {
                            format!("{}/avatar.png", base_url)
                        };

                        let body = json!({
                            "code": 0,
                            "data": {
                                "name": "Test Character",
                                "description": "Test Description",
                                "scenario": "Test Scenario",
                                "firstMes": "Hello!",
                                "picture": picture,
                                "extra": {"tags": ["TestTag"]}
                            }
                        });
                        Json(body).into_response()
                    }
                }),
            )
            .route("/avatar.png", get(avatar_png));

        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("mock server failed");
        });

        Self {
            base_url,
            _task: task,
        }
    }
}
