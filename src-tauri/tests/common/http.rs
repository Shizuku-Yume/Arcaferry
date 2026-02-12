#![allow(dead_code)]

use axum::{
    body::{to_bytes, Body},
    http::{header, Method, Request, Response},
};

pub fn json_request(uri: &str, method: Method, body: serde_json::Value) -> Request<Body> {
    let bytes = serde_json::to_vec(&body).expect("failed to serialize json request");
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(bytes))
        .expect("failed to build request")
}

pub async fn read_json_response(res: Response<Body>) -> serde_json::Value {
    let bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("failed to read response body");
    serde_json::from_slice(&bytes).expect("failed to parse response json")
}
