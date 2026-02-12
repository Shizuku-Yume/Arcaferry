use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;
use strum::VariantArray;
use wreq::{
    header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, AUTHORIZATION, COOKIE},
    Client,
};
use wreq_util::{Emulation, EmulationOS, EmulationOption};

use crate::cookies::CookieJar;
use crate::error::{ArcaferryError, Result};

const DEFAULT_TIMEOUT_SECS: u64 = 30;

pub struct HttpClient {
    client: Client,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrowserVersionRange {
    pub min: u32,
    pub max: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SupportedBrowsers {
    pub chrome: Option<BrowserVersionRange>,
    pub edge: Option<BrowserVersionRange>,
    pub firefox: Option<BrowserVersionRange>,
    pub safari: Option<BrowserVersionRange>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionWarning {
    pub browser: String,
    pub user_version: u32,
    pub max_supported: u32,
    pub message: String,
    pub update_command: String,
}

pub fn get_supported_browsers() -> SupportedBrowsers {
    let mut ranges: HashMap<&str, (u32, u32)> = HashMap::new();

    for &emulation in Emulation::VARIANTS {
        if let Some((browser, version)) = parse_emulation_variant(emulation) {
            let entry = ranges.entry(browser).or_insert((version, version));
            entry.0 = entry.0.min(version);
            entry.1 = entry.1.max(version);
        }
    }

    let to_range = |browser: &str| -> Option<BrowserVersionRange> {
        ranges.get(browser).map(|(min, max)| BrowserVersionRange {
            min: *min,
            max: *max,
        })
    };

    SupportedBrowsers {
        chrome: to_range("Chrome"),
        edge: to_range("Edge"),
        firefox: to_range("Firefox"),
        safari: to_range("Safari"),
    }
}

pub fn check_version_warning(ua: Option<&str>) -> Option<VersionWarning> {
    let ua_str = ua.unwrap_or("");
    let supported = get_supported_browsers();

    if let Ok(edge_re) = Regex::new(r"Edg/(\d+)") {
        if let Some(caps) = edge_re.captures(ua_str) {
            if let Some(ver) = caps.get(1).and_then(|m| m.as_str().parse::<u32>().ok()) {
                if let Some(ref range) = supported.edge {
                    if ver > range.max {
                        return Some(build_warning("Edge", ver, range.max));
                    }
                }
            }
        }
    }

    if let Ok(chrome_re) = Regex::new(r"Chrome/(\d+)") {
        if let Some(caps) = chrome_re.captures(ua_str) {
            if let Some(ver) = caps.get(1).and_then(|m| m.as_str().parse::<u32>().ok()) {
                if let Some(ref range) = supported.chrome {
                    if ver > range.max {
                        return Some(build_warning("Chrome", ver, range.max));
                    }
                }
            }
        }
    }

    None
}

fn build_warning(browser: &str, user_version: u32, max_supported: u32) -> VersionWarning {
    VersionWarning {
        browser: browser.to_string(),
        user_version,
        max_supported,
        message: format!(
            "Your {} version ({}) is newer than the maximum supported version ({}). TLS fingerprint mismatch may cause Cloudflare blocks.",
            browser, user_version, max_supported
        ),
        update_command: "cd Arcaferry/src-tauri && cargo update && cargo build --release".to_string(),
    }
}

/// Extract version number from Emulation variant name (e.g., "Chrome143" -> Some(("Chrome", 143)))
fn parse_emulation_variant(emulation: Emulation) -> Option<(&'static str, u32)> {
    let name = format!("{:?}", emulation);

    // Try to extract browser type and version from variant name
    let patterns = [
        ("Chrome", "Chrome"),
        ("Edge", "Edge"),
        ("Firefox", "Firefox"),
        ("Safari", "Safari"),
        ("Opera", "Opera"),
    ];

    for (prefix, browser_type) in patterns {
        if let Some(version_str) = name.strip_prefix(prefix) {
            // Handle variants like "Chrome143", "Safari18_3" (take first number)
            let version: String = version_str
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(ver) = version.parse::<u32>() {
                return Some((browser_type, ver));
            }
        }
    }
    None
}

/// Find the closest matching Emulation variant for a given browser type and version
fn find_closest_emulation(browser_type: &str, target_version: u32) -> Emulation {
    let mut best_match: Option<(Emulation, i32)> = None;

    for &emulation in Emulation::VARIANTS {
        if let Some((emu_browser, emu_version)) = parse_emulation_variant(emulation) {
            if emu_browser == browser_type {
                let distance = (emu_version as i32 - target_version as i32).abs();
                if best_match.is_none() || distance < best_match.unwrap().1 {
                    best_match = Some((emulation, distance));
                }
            }
        }
    }

    // Fallback: find the latest Chrome version available
    best_match.map(|(e, _)| e).unwrap_or_else(|| {
        Emulation::VARIANTS
            .iter()
            .filter_map(|&e| {
                parse_emulation_variant(e)
                    .filter(|(browser, _)| *browser == "Chrome")
                    .map(|(_, ver)| (e, ver))
            })
            .max_by_key(|(_, ver)| *ver)
            .map(|(e, _)| e)
            .unwrap_or(Emulation::VARIANTS[0])
    })
}

pub fn parse_user_agent(ua: Option<&str>) -> (Emulation, EmulationOS) {
    let ua_str = ua.unwrap_or("");

    let os = if ua_str.contains("Macintosh") || ua_str.contains("Mac OS") {
        EmulationOS::MacOS
    } else if ua_str.contains("Linux") && !ua_str.contains("Android") {
        EmulationOS::Linux
    } else {
        EmulationOS::Windows
    };

    // Check for Edge first (Edge UA contains both "Chrome" and "Edg")
    let edge_re = Regex::new(r"Edg/(\d+)").ok();
    if let Some(ref re) = edge_re {
        if let Some(caps) = re.captures(ua_str) {
            if let Some(ver) = caps.get(1).and_then(|m| m.as_str().parse::<u32>().ok()) {
                return (find_closest_emulation("Edge", ver), os);
            }
        }
    }

    // Check for Chrome
    let chrome_re = Regex::new(r"Chrome/(\d+)").ok();
    if let Some(ref re) = chrome_re {
        if let Some(caps) = re.captures(ua_str) {
            if let Some(ver) = caps.get(1).and_then(|m| m.as_str().parse::<u32>().ok()) {
                return (find_closest_emulation("Chrome", ver), os);
            }
        }
    }

    // Check for Firefox
    if ua_str.contains("Firefox") {
        let firefox_re = Regex::new(r"Firefox/(\d+)").ok();
        if let Some(ref re) = firefox_re {
            if let Some(caps) = re.captures(ua_str) {
                if let Some(ver) = caps.get(1).and_then(|m| m.as_str().parse::<u32>().ok()) {
                    return (find_closest_emulation("Firefox", ver), os);
                }
            }
        }
        // Fallback to latest Firefox
        return (find_closest_emulation("Firefox", 999), os);
    }

    // Check for Safari (but not Chrome-based browsers that also contain "Safari")
    if ua_str.contains("Safari") && !ua_str.contains("Chrome") {
        let safari_re = Regex::new(r"Version/(\d+)").ok();
        if let Some(ref re) = safari_re {
            if let Some(caps) = re.captures(ua_str) {
                if let Some(ver) = caps.get(1).and_then(|m| m.as_str().parse::<u32>().ok()) {
                    return (find_closest_emulation("Safari", ver), os);
                }
            }
        }
        // Fallback to latest Safari
        return (find_closest_emulation("Safari", 999), os);
    }

    // Default: use the latest Chrome version available
    (find_closest_emulation("Chrome", 999), os)
}

impl HttpClient {
    pub fn new() -> Result<Self> {
        Self::with_config(None, None, None, None)
    }

    pub fn with_config(
        cookies: Option<&CookieJar>,
        bearer_token: Option<&str>,
        timeout_secs: Option<u64>,
        user_agent: Option<&str>,
    ) -> Result<Self> {
        let mut headers = HeaderMap::new();

        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json, text/plain, */*"),
        );
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));

        if let Some(jar) = cookies {
            let cookie_str = jar.to_header_string();
            if !cookie_str.is_empty() {
                headers.insert(
                    COOKIE,
                    HeaderValue::from_str(&cookie_str)
                        // Avoid echoing cookie content in errors/logs.
                        .map_err(|_| {
                            ArcaferryError::ValidationError("Invalid cookies".to_string())
                        })?,
                );
            }
        }

        if let Some(token) = bearer_token {
            let clean_token = token
                .strip_prefix("Bearer ")
                .or_else(|| token.strip_prefix("bearer "))
                .unwrap_or(token)
                .trim();
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", clean_token))
                    // Avoid echoing token content in errors/logs.
                    .map_err(|_| {
                        ArcaferryError::ValidationError("Invalid bearer_token".to_string())
                    })?,
            );
        }

        let timeout = Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS));

        let (emulation, os) = parse_user_agent(user_agent);
        let emulation_opt = EmulationOption::builder()
            .emulation(emulation)
            .emulation_os(os)
            .build();

        let client = Client::builder()
            .emulation(emulation_opt)
            .default_headers(headers)
            .timeout(timeout)
            .build()
            .map_err(|e| ArcaferryError::NetworkError(e.to_string()))?;

        Ok(Self { client })
    }

    pub async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| self.classify_error(e))?;

        self.handle_response(response).await
    }

    pub async fn get_text(&self, url: &str) -> Result<String> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| self.classify_error(e))?;

        self.handle_response_text(response).await
    }

    pub async fn post_json<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        &self,
        url: &str,
        body: &B,
    ) -> Result<T> {
        let response = self
            .client
            .post(url)
            .json(body)
            .send()
            .await
            .map_err(|e| self.classify_error(e))?;

        self.handle_response(response).await
    }

    pub async fn post_text<B: serde::Serialize>(&self, url: &str, body: &B) -> Result<String> {
        let response = self
            .client
            .post(url)
            .json(body)
            .send()
            .await
            .map_err(|e| self.classify_error(e))?;

        self.handle_response_text(response).await
    }

    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: wreq::Response,
    ) -> Result<T> {
        let status = response.status();

        if status.is_success() {
            let body = response
                .text()
                .await
                .map_err(|e| ArcaferryError::NetworkError(e.to_string()))?;

            if body.trim_start().starts_with("<!DOCTYPE") || body.trim_start().starts_with("<html")
            {
                if body.contains("Just a moment")
                    || body.contains("cf_chl_opt")
                    || body.contains("cloudflare")
                {
                    return Err(ArcaferryError::CloudflareBlocked);
                }
                return Err(ArcaferryError::InvalidJson(
                    "Received HTML instead of JSON".to_string(),
                ));
            }

            serde_json::from_str(&body).map_err(|e| ArcaferryError::InvalidJson(e.to_string()))
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(self.status_to_error(status, &body))
        }
    }

    async fn handle_response_text(&self, response: wreq::Response) -> Result<String> {
        let status = response.status();

        let body = response
            .text()
            .await
            .map_err(|e| ArcaferryError::NetworkError(e.to_string()))?;

        if status.is_success() {
            if body.trim_start().starts_with("<!DOCTYPE") || body.trim_start().starts_with("<html")
            {
                if body.contains("Just a moment")
                    || body.contains("cf_chl_opt")
                    || body.contains("cloudflare")
                {
                    return Err(ArcaferryError::CloudflareBlocked);
                }
                return Err(ArcaferryError::InvalidJson(
                    "Received HTML instead of JSON".to_string(),
                ));
            }

            Ok(body)
        } else {
            Err(self.status_to_error(status, &body))
        }
    }

    fn classify_error(&self, error: wreq::Error) -> ArcaferryError {
        if error.is_timeout() {
            ArcaferryError::Timeout(error.to_string())
        } else if error.is_connect() {
            ArcaferryError::NetworkError(format!("Connection failed: {}", error))
        } else {
            ArcaferryError::NetworkError(error.to_string())
        }
    }

    fn status_to_error(&self, status: wreq::StatusCode, body: &str) -> ArcaferryError {
        let body = if body.trim().is_empty() {
            "Authentication required. Provide cookies and/or bearer_token (Cloudflare may require cf_clearance)."
        } else {
            body
        };

        let looks_like_cloudflare = body.contains("Just a moment")
            || body.contains("cf_chl_opt")
            || body.contains("cloudflare")
            || body.contains("cf-");

        match status.as_u16() {
            401 => ArcaferryError::Unauthorized(body.to_string()),
            403 => {
                if looks_like_cloudflare {
                    ArcaferryError::CloudflareBlocked
                } else {
                    ArcaferryError::Unauthorized(body.to_string())
                }
            }
            429 => ArcaferryError::RateLimited(60),
            503 => {
                if looks_like_cloudflare {
                    ArcaferryError::CloudflareBlocked
                } else {
                    ArcaferryError::NetworkError(format!("HTTP {}: {}", status, body))
                }
            }
            _ => {
                if looks_like_cloudflare {
                    ArcaferryError::CloudflareBlocked
                } else {
                    ArcaferryError::NetworkError(format!("HTTP {}: {}", status, body))
                }
            }
        }
    }

    pub fn inner(&self) -> &Client {
        &self.client
    }

    /// Post JSON and collect SSE stream response into a single string.
    /// The stream is expected to be Server-Sent Events format with `data:` prefixes.
    pub async fn post_sse_stream<B: serde::Serialize>(
        &self,
        url: &str,
        body: &B,
    ) -> Result<String> {
        use futures::StreamExt;

        let response = self
            .client
            .post(url)
            .json(body)
            .send()
            .await
            .map_err(|e| self.classify_error(e))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(self.status_to_error(status, &body));
        }

        let mut stream = response.bytes_stream();
        let mut collected = String::new();
        let mut chunk_count = 0;
        let mut buffer = String::new();

        // Debug: capture raw SSE data
        let mut raw_sse_log = String::new();
        
        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    chunk_count += 1;
                    let chunk_str = String::from_utf8_lossy(&chunk);
                    tracing::debug!(chunk_count, chunk_len = chunk.len(), "SSE chunk received");
                    
                    // Log raw chunk for debugging
                    raw_sse_log.push_str(&format!("--- CHUNK {} (len={}) ---\n{}\n", chunk_count, chunk.len(), chunk_str));
                    
                    buffer.push_str(&chunk_str);
                    
                    // Process complete lines from buffer
                    while let Some(newline_pos) = buffer.find('\n') {
                        let line = buffer[..newline_pos].to_string();
                        buffer = buffer[newline_pos + 1..].to_string();
                        
                        if let Some(data) = line.strip_prefix("data: ") {
                            if data.trim() == "[DONE]" {
                                tracing::debug!("SSE stream [DONE] received");
                                continue;
                            }
                            // Try to parse as JSON and extract content
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                                // OpenAI-style: choices[0].delta.content
                                if let Some(content) = json
                                    .get("choices")
                                    .and_then(|c| c.get(0))
                                    .and_then(|c| c.get("delta"))
                                    .and_then(|d| d.get("content"))
                                    .and_then(|c| c.as_str())
                                {
                                    collected.push_str(content);
                                }
                                // Quack-style: content field directly
                                else if let Some(content) = json.get("content").and_then(|c| c.as_str()) {
                                    collected.push_str(content);
                                }
                            } else {
                                tracing::debug!(data_preview = %data.chars().take(100).collect::<String>(), "SSE JSON parse failed");
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("SSE stream error: {}", e);
                    break;
                }
            }
        }

        tracing::debug!(chunk_count, collected_len = collected.len(), "SSE stream finished");
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let _ = std::fs::write(format!("/tmp/sse_raw_debug_{}.txt", timestamp), &raw_sse_log);
        Ok(collected)
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default HTTP client")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_to_error_unauthorized_blank_body_is_actionable() {
        let client = HttpClient::new().unwrap();
        match client.status_to_error(wreq::StatusCode::from_u16(401).unwrap(), "") {
            ArcaferryError::Unauthorized(msg) => {
                assert!(msg.contains("Authentication required"));
                assert!(msg.contains("cf_clearance"));
            }
            other => panic!("expected Unauthorized, got: {:?}", other),
        }
    }

    #[test]
    fn test_status_to_error_unauthorized_non_blank_body_preserved() {
        let client = HttpClient::new().unwrap();
        let body = "missing token";
        match client.status_to_error(wreq::StatusCode::from_u16(401).unwrap(), body) {
            ArcaferryError::Unauthorized(msg) => assert_eq!(msg, body),
            other => panic!("expected Unauthorized, got: {:?}", other),
        }
    }

    #[test]
    fn test_status_to_error_cloudflare_detection_on_403() {
        let client = HttpClient::new().unwrap();
        match client.status_to_error(wreq::StatusCode::from_u16(403).unwrap(), "cloudflare") {
            ArcaferryError::CloudflareBlocked => {}
            other => panic!("expected CloudflareBlocked, got: {:?}", other),
        }
    }

    #[test]
    fn test_status_to_error_cloudflare_detection_on_503() {
        let client = HttpClient::new().unwrap();
        match client.status_to_error(
            wreq::StatusCode::from_u16(503).unwrap(),
            "<!DOCTYPE html><title>Just a moment...</title>",
        ) {
            ArcaferryError::CloudflareBlocked => {}
            other => panic!("expected CloudflareBlocked, got: {:?}", other),
        }
    }
}
