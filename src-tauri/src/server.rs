use crate::adapters::quack::{
    collect_all_attrs, extract_quack_id, get_api_base, get_url_type, map_lorebook, map_quack_to_v3,
    get_hidden_attr_labels, has_empty_hidden_settings, QuackAttribute, QuackCharacterInfo, QuackClient,
    QuackLorebookEntry, AVATAR_BASE_URL,
};
use crate::browser_sidecar::{
    detect_browser_capability, extract_hidden_settings_via_sidecar, BrowserCapability,
    SidecarHiddenSettings,
};
use crate::ccv3::{CharacterCardV3, Lorebook};
use crate::cookies::CookieJar;

use crate::error::ArcaferryError;
use crate::http::{
    check_version_warning, get_supported_browsers, parse_user_agent, SupportedBrowsers, VersionWarning,
};
use crate::png_export::create_card_png;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, info, warn};

const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ImportMode {
    #[default]
    Full,
    OnlyLorebook,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    #[default]
    Json,
    Png,
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub status: String,
    pub version: String,
    pub ready: bool,
    pub port: u16,
    pub supported_browsers: SupportedBrowsers,
    pub browser_extraction_available: bool,
    pub browser_extraction_reason: String,
}

#[derive(Debug, Deserialize)]
pub struct ScrapeRequest {
    pub url: String,
    pub cookies: Option<String>,
    pub bearer_token: Option<String>,
    pub user_agent: Option<String>,
    pub gemini_api_key: Option<String>,
    #[serde(default)]
    pub output_format: OutputFormat,
}

#[derive(Debug, Serialize)]
pub struct ScrapeResponse {
    pub success: bool,
    pub card: Option<CharacterCardV3>,
    pub avatar_base64: Option<String>,
    pub png_base64: Option<String>,
    pub warnings: Vec<String>,
    pub error: Option<String>,
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_warning: Option<VersionWarning>,
}

fn default_concurrency() -> usize {
    3
}

#[derive(Debug, Deserialize)]
pub struct BatchScrapeRequest {
    pub urls: Vec<String>,
    pub cookies: Option<String>,
    pub bearer_token: Option<String>,
    pub user_agent: Option<String>,
    pub gemini_api_key: Option<String>,
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
    #[serde(default)]
    pub output_format: OutputFormat,
}

#[derive(Debug, Serialize, Clone)]
pub struct BatchItemResult {
    pub url: String,
    pub success: bool,
    pub card: Option<CharacterCardV3>,
    pub avatar_base64: Option<String>,
    pub png_base64: Option<String>,
    pub warnings: Vec<String>,
    pub error: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BatchScrapeResponse {
    pub success: bool,
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub results: Vec<BatchItemResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_warning: Option<VersionWarning>,
}

#[derive(Debug, Deserialize)]
pub struct ImportRequest {
    pub quack_input: String,
    pub lorebook_json: Option<String>,
    pub cookies: Option<String>,
    pub bearer_token: Option<String>,
    pub user_agent: Option<String>,
    pub gemini_api_key: Option<String>,
    #[serde(default)]
    pub mode: ImportMode,
    #[serde(default)]
    pub output_format: OutputFormat,
}

#[derive(Debug, Serialize)]
pub struct ImportResponse {
    pub success: bool,
    pub card: Option<CharacterCardV3>,
    pub lorebook: Option<Lorebook>,
    pub png_base64: Option<String>,
    pub avatar_base64: Option<String>,
    pub source: String,
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_warning: Option<VersionWarning>,
}

#[derive(Debug, Deserialize)]
pub struct PreviewRequest {
    pub quack_input: String,
    pub cookies: Option<String>,
    pub bearer_token: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PreviewData {
    pub name: String,
    pub creator: String,
    pub intro: String,
    pub tags: Vec<String>,
    pub attr_count: i32,
    pub lorebook_count: i32,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct PreviewResponse {
    pub success: bool,
    pub data: Option<PreviewData>,
    pub error: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
    pub error_code: String,
}

#[derive(Clone)]
pub struct ServerState {
    pub port: u16,
    pub browser_capability: BrowserCapability,
    /// Optional API base override for integration tests.
    ///
    /// When set, all Quack/Purrly API calls will be routed to this base URL
    /// instead of the built-in production hosts (https://purrly.ai / https://quack.im).
    pub api_base_override: Option<String>,

    /// Optional HTTP request timeout override (seconds) for Quack/Purrly API calls.
    ///
    /// This exists primarily for hermetic integration tests to validate TIMEOUT
    /// error mapping without waiting for the production default timeout.
    ///
    /// NOTE: This does *not* affect avatar fetching (which has its own timeout).
    pub http_timeout_secs_override: Option<u64>,
}

fn effective_api_base<'a>(state: &'a ServerState, input: &str) -> &'a str {
    state
        .api_base_override
        .as_deref()
        .unwrap_or_else(|| get_api_base(input))
}

fn browser_capability_fields(cap: &BrowserCapability) -> (bool, String) {
    match cap {
        BrowserCapability::Available => (true, "Python + Camoufox detected".to_string()),
        BrowserCapability::NotInstalled { reason } => (false, reason.clone()),
        BrowserCapability::Error { reason } => (false, reason.clone()),
    }
}

fn apply_hidden_settings(info: &mut QuackCharacterInfo, extracted: &[QuackAttribute]) -> usize {
    // Sidecar returns a list of {label|name, value} pairs. Apply them back into the original
    // Quack info *without losing unknown JSON fields*.

    fn build_value_map(extracted: &[QuackAttribute]) -> HashMap<String, String> {
        let mut map: HashMap<String, String> = HashMap::new();
        for ext in extracted {
            let value = ext.value.as_deref().unwrap_or("").trim();
            if value.is_empty() {
                continue;
            }
            if let Some(label) = ext.label.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                map.insert(label.to_string(), value.to_string());
            }
            if let Some(name) = ext.name.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                // Only fill if absent; label should win when both exist.
                map.entry(name.to_string()).or_insert_with(|| value.to_string());
            }
        }
        map
    }

    fn apply_to_vec(attrs: &mut [QuackAttribute], values: &HashMap<String, String>) -> usize {
        let mut applied = 0usize;
        for target in attrs.iter_mut() {
            if target.is_visible != Some(false) {
                continue;
            }
            if !target.value.as_deref().unwrap_or("").trim().is_empty() {
                continue;
            }

            let label = target.label.as_deref().unwrap_or("").trim();
            let name = target.name.as_deref().unwrap_or("").trim();

            let found = if !label.is_empty() {
                values.get(label)
            } else if !name.is_empty() {
                values.get(name)
            } else {
                None
            };

            if let Some(v) = found {
                target.value = Some(v.clone());
                applied += 1;
            }
        }
        applied
    }

    fn apply_to_value_array(attrs: &mut serde_json::Value, values: &HashMap<String, String>) -> usize {
        let Some(arr) = attrs.as_array_mut() else {
            return 0;
        };

        let mut applied = 0usize;
        for item in arr.iter_mut() {
            let Some(obj) = item.as_object_mut() else {
                continue;
            };

            // Only patch hidden + empty.
            let is_visible = obj.get("isVisible").and_then(|v| v.as_bool());
            if is_visible != Some(false) {
                continue;
            }

            let current_value = obj
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if !current_value.is_empty() {
                continue;
            }

            let label = obj
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let name = obj
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();

            let found = if !label.is_empty() {
                values.get(label)
            } else if !name.is_empty() {
                values.get(name)
            } else {
                None
            };

            if let Some(v) = found {
                obj.insert("value".to_string(), serde_json::Value::String(v.clone()));
                applied += 1;
            }
        }

        applied
    }

    let values = build_value_map(extracted);
    if values.is_empty() {
        return 0;
    }

    let mut applied = 0usize;

    // 1) Top-level `customAttrs` (serde_json::Value array)
    if let Some(ref mut v) = info.custom_attrs {
        applied += apply_to_value_array(v, &values);
    }

    // 2) charList[0] attrs/advise/custom
    if let Some(ref mut char_list) = info.char_list {
        if let Some(first) = char_list.first_mut() {
            if let Some(ref mut attrs) = first.attrs {
                applied += apply_to_vec(attrs, &values);
            }
            if let Some(ref mut attrs) = first.advise_attrs {
                applied += apply_to_vec(attrs, &values);
            }
            if let Some(ref mut attrs) = first.custom_attrs {
                applied += apply_to_vec(attrs, &values);
            }
        }
    }

    applied
}

fn sidecar_debug_warning(sidecar: &SidecarHiddenSettings) -> Option<String> {
    let s = sidecar.stderr.trim();
    if s.is_empty() {
        return None;
    }

    // Keep it single-line and short.
    let one_line = s.replace('\n', " | ");
    let truncated = if one_line.len() > 500 {
        let head = &one_line[..250];
        let tail = &one_line[one_line.len() - 250..];
        format!("{}…{}", head, tail)
    } else {
        one_line
    };
    Some(format!("sidecar debug: {}", truncated))
}

fn try_parse_json_object(input: &str) -> Option<serde_json::Value> {
    let trimmed = input.trim();
    let trimmed = trimmed.strip_prefix('\u{feff}').unwrap_or(trimmed);
    if !trimmed.starts_with('{') {
        return None;
    }
    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(v) if v.is_object() => Some(v),
        _ => None,
    }
}

fn parse_manual_quack_json(json_value: &serde_json::Value) -> Result<QuackCharacterInfo, ArcaferryError> {
    serde_json::from_value(json_value.clone())
        .map_err(|e| ArcaferryError::InvalidJson(format!("Failed to parse Quack data: {}", e)))
}

fn extract_lorebook_entries_from_json(json_value: &serde_json::Value) -> Vec<QuackLorebookEntry> {
    let mut entries = Vec::new();

    if let Some(data) = json_value.get("data").and_then(|v| v.as_array()) {
        for book in data {
            if let Some(entry_list) = book.get("entryList").and_then(|v| v.as_array()) {
                for entry in entry_list {
                    if let Ok(e) = serde_json::from_value::<QuackLorebookEntry>(entry.clone()) {
                        entries.push(e);
                    }
                }
            }
        }
        if !entries.is_empty() {
            return entries;
        }
    }

    if let Some(arr) = json_value.as_array() {
        for book in arr {
            if let Some(entry_list) = book.get("entryList").and_then(|v| v.as_array()) {
                for entry in entry_list {
                    if let Ok(e) = serde_json::from_value::<QuackLorebookEntry>(entry.clone()) {
                        entries.push(e);
                    }
                }
            }
        }
        if !entries.is_empty() {
            return entries;
        }
    }

    if let Some(books) = json_value.get("characterbooks").and_then(|v| v.as_array()) {
        for book in books {
            if let Some(entry_list) = book.get("entryList").and_then(|v| v.as_array()) {
                for entry in entry_list {
                    if let Ok(e) = serde_json::from_value::<QuackLorebookEntry>(entry.clone()) {
                        entries.push(e);
                    }
                }
            }
        }
    }

    entries
}

fn extract_preview_from_quack(info: &QuackCharacterInfo) -> PreviewData {
    let char_name = info
        .char_list
        .as_ref()
        .and_then(|list| list.first())
        .and_then(|c| c.name.clone())
        .unwrap_or_else(|| info.name.clone());

    let creator = info
        .author_name
        .clone()
        .or_else(|| info.creator.clone())
        .unwrap_or_default();

    let intro = info
        .intro
        .clone()
        .or_else(|| info.description.clone())
        .map(|s| {
            if s.len() > 200 {
                format!("{}...", &s[..197])
            } else {
                s
            }
        })
        .unwrap_or_default();

    let tags: Vec<String> = info
        .extra
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .take(10)
                .collect()
        })
        .unwrap_or_default();

    let attr_count = collect_all_attrs(info).len() as i32;

    let lorebook_count = info
        .characterbooks
        .as_ref()
        .map(|books| {
            books
                .iter()
                .filter_map(|b| b.entry_list.as_ref())
                .map(|entries| entries.len())
                .sum::<usize>()
        })
        .unwrap_or(0) as i32;

    PreviewData {
        name: char_name,
        creator,
        intro,
        tags,
        attr_count,
        lorebook_count,
        source: String::new(),
    }
}

fn error_to_http(e: &ArcaferryError) -> (StatusCode, &'static str) {
    match e {
        ArcaferryError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
        ArcaferryError::RateLimited(_) => (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED"),
        ArcaferryError::Timeout(_) => (StatusCode::GATEWAY_TIMEOUT, "TIMEOUT"),
        ArcaferryError::CloudflareBlocked => (StatusCode::SERVICE_UNAVAILABLE, "CLOUDFLARE_BLOCKED"),
        ArcaferryError::InvalidUrl(_) => (StatusCode::BAD_REQUEST, "INVALID_URL"),
        ArcaferryError::InvalidJson(_) => (StatusCode::BAD_REQUEST, "PARSE_ERROR"),
        ArcaferryError::ValidationError(_) => (StatusCode::BAD_REQUEST, "PARSE_ERROR"),
        _ => (StatusCode::BAD_GATEWAY, "NETWORK_ERROR"),
    }
}

fn has_cf_clearance_cookie(cookies_raw: Option<&str>, cookie_jar: Option<&CookieJar>) -> bool {
    if let Some(jar) = cookie_jar {
        if jar.get("cf_clearance").is_some() {
            return true;
        }
    }

    cookies_raw
        .map(|s| s.contains("cf_clearance="))
        .unwrap_or(false)
}

fn cloudflare_auth_guidance(has_cf_clearance: bool, has_user_agent: bool) -> String {
    // Keep this message actionable and safe (do not echo cookie/token values).
    let mut lines: Vec<String> = Vec::new();

    if !has_cf_clearance {
        lines.push("Missing Cloudflare cookie: cf_clearance (required).".to_string());
    }
    if !has_user_agent {
        lines.push("Missing request field: user_agent (must match the browser that generated cf_clearance).".to_string());
    }

    lines.push("".to_string());
    lines.push("How to fix Cloudflare verification:".to_string());
    lines.push("1) Use a real browser to open the target site until the challenge passes.".to_string());
    lines.push("2) Copy the cf_clearance cookie from THAT browser profile/session.".to_string());
    lines.push("   - Chrome/Edge: DevTools → Application → Storage → Cookies → https://quack.im (or https://purrly.ai)".to_string());
    lines.push("   - Or Network tab: request headers → Cookie".to_string());
    lines.push("3) Send the SAME browser User-Agent via user_agent.".to_string());
    lines.push("   - In browser console: navigator.userAgent".to_string());
    lines.push("4) Retry the API call with BOTH cookies and user_agent.".to_string());
    lines.push("".to_string());
    lines.push("Notes:".to_string());
    lines.push("- cf_clearance is bound to your UA/TLS fingerprint and IP. If you changed UA/VPN/network, get a fresh cf_clearance.".to_string());
    lines.push("- Check GET /api/status for supported_browsers and use a desktop UA within that range.".to_string());

    lines.join("\n")
}

fn maybe_attach_cloudflare_guidance(
    e: &ArcaferryError,
    error_code: &str,
    has_cf_clearance: bool,
    has_user_agent: bool,
) -> String {
    if error_code == "CLOUDFLARE_BLOCKED" {
        let guidance = cloudflare_auth_guidance(has_cf_clearance, has_user_agent);
        format!("{}\n\n{}", e, guidance)
    } else {
        e.to_string()
    }
}

async fn status_handler(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    let (browser_extraction_available, browser_extraction_reason) =
        browser_capability_fields(&state.browser_capability);
    Json(StatusResponse {
        status: "ok".to_string(),
        version: SERVER_VERSION.to_string(),
        ready: true,
        port: state.port,
        supported_browsers: get_supported_browsers(),
        browser_extraction_available,
        browser_extraction_reason,
    })
}

async fn scrape_handler(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<ScrapeRequest>,
) -> Result<Json<ScrapeResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut warnings: Vec<String> = Vec::new();
    let cookie_jar = request.cookies.as_ref().and_then(|c| CookieJar::parse(c).ok());
    let api_base = effective_api_base(&state, &request.url);
    let http_timeout_secs = state.http_timeout_secs_override.unwrap_or(30);
    let url_type = get_url_type(&request.url);

    let id = extract_quack_id(&request.url).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                error: e.to_string(),
                error_code: "INVALID_URL".to_string(),
            }),
        )
    })?;

    let client = QuackClient::new_with_timeout(
        cookie_jar.as_ref(),
        request.bearer_token.as_deref(),
        Some(api_base),
        request.user_agent.as_deref(),
        http_timeout_secs,
    )
    .map_err(|e| {
        let (status, code) = error_to_http(&e);
        (
            status,
            Json(ErrorResponse {
                success: false,
                error: e.to_string(),
                error_code: code.to_string(),
            }),
        )
    })?;

    let api_result = client.fetch_complete_with_type(&id, url_type).await;

    let (mut info, lorebook, chat_index) = match api_result {
        Ok((info, lorebook, chat_index)) => (info, lorebook, chat_index),
        Err(e) => {
            let (status, code) = error_to_http(&e);
            let has_user_agent = request
                .user_agent
                .as_deref()
                .is_some_and(|ua| !ua.trim().is_empty());
            let has_cf_clearance =
                has_cf_clearance_cookie(request.cookies.as_deref(), cookie_jar.as_ref());
            let error_message =
                maybe_attach_cloudflare_guidance(&e, code, has_cf_clearance, has_user_agent);
            return Err((
                status,
                Json(ErrorResponse {
                    success: false,
                    error: error_message,
                    error_code: code.to_string(),
                }),
            ));
        }
    };

    let needs_hidden = has_empty_hidden_settings(&info);
    debug!(?url_type, needs_hidden, "scrape requirements");

    if needs_hidden {
        match &state.browser_capability {
            BrowserCapability::Available => {
                let hidden_labels = get_hidden_attr_labels(&info);
                info!(hidden_labels_count = hidden_labels.len(), "sidecar hidden extraction check");

                if hidden_labels.is_empty() {
                    warnings.push(
                        "检测到隐藏设定但未找到可提取的隐藏属性标签".to_string(),
                    );
                    warn!("sidecar skipped: no hidden labels");
                    // Skip sidecar call.
                } else {

                // Prefer share URL for browser sidecar.
                let mut share_url = request.url.clone();
                if !share_url.starts_with("http") {
                    share_url = format!("{}/discovery/share/{}", api_base, id);
                } else if url_type == crate::adapters::quack::QuackUrlType::Dream {
                    if let Ok(chat_info) = client.fetch_chat_info_by_index(&id).await {
                        let origin_sid = chat_info
                            .extra
                            .get("originSid")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if !origin_sid.is_empty() {
                            share_url = format!("{}/discovery/share/{}", api_base, origin_sid);
                        }
                    }
                }

                let dream_url = chat_index.as_ref().map(|idx| format!("{}/dream/{}", api_base, idx));

                info!("invoking sidecar for hidden settings");
                match extract_hidden_settings_via_sidecar(
                    &share_url,
                    &hidden_labels,
                    crate::browser_sidecar::SidecarInvokeParams {
                        cookies: request.cookies.as_deref(),
                        bearer_token: request.bearer_token.as_deref(),
                        gemini_api_key: request.gemini_api_key.as_deref(),
                        user_agent: request.user_agent.as_deref(),
                        dream_url: dream_url.as_deref(),
                    },
                )
                .await
                {
                    Ok(sidecar) => {
                        let applied = apply_hidden_settings(&mut info, &sidecar.attrs);
                        info!(attrs_count = sidecar.attrs.len(), applied, "sidecar returned");
                        if applied > 0 {
                            warnings.push("已通过 sidecar 提取隐藏设定".to_string());
                        } else {
                            warnings.push("检测到隐藏设定但 sidecar 未返回有效内容".to_string());
                            warn!("sidecar returned but nothing applied");
                            if let Some(w) = sidecar_debug_warning(&sidecar) {
                                warnings.push(w);
                            }
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "sidecar failed");
                        warnings.push(format!("隐藏设定提取失败: {}", e));
                    }
                }
                }
            }
            BrowserCapability::NotInstalled { reason } | BrowserCapability::Error { reason } => {
                info!(reason = %reason, "sidecar unavailable");
                warnings.push(format!(
                    "隐藏设定提取不可用: {}。基础数据已提取成功。",
                    reason
                ));
            }
        }
    }

    let avatar_base64 = if let Some(ref picture) = info.picture {
        let avatar_url = if picture.starts_with("http") {
            picture.clone()
        } else {
            format!("{}{}", AVATAR_BASE_URL, picture)
        };

        match fetch_avatar_base64(&avatar_url, request.user_agent.as_deref()).await {
            Ok(b64) => Some(b64),
            Err(e) => {
                // Non-fatal: return card without avatar.
                // Keep error message generic and safe.
                warn!(error = %e, "avatar fetch failed");
                warnings.push("封面图片下载失败".to_string());
                None
            }
        }
    } else {
        info!("no picture field in quack info; avatar will be empty");
        None
    };

    let card = map_quack_to_v3(&info, &lorebook);
    debug!(system_prompt_len = card.data.system_prompt.len(), warnings_count = warnings.len(), "scrape mapped card");

    let png_base64 = if request.output_format == OutputFormat::Png {
        create_card_png(&card, avatar_base64.as_deref())
            .ok()
            .map(|bytes| BASE64.encode(&bytes))
    } else {
        None
    };

    let version_warning = check_version_warning(request.user_agent.as_deref());

    Ok(Json(ScrapeResponse {
        success: true,
        card: Some(card),
        avatar_base64: if request.output_format == OutputFormat::Json {
            avatar_base64
        } else {
            None
        },
        png_base64,
        warnings,
        error: None,
        error_code: None,
        version_warning,
    }))
}

async fn fetch_avatar_base64(url: &str, user_agent: Option<&str>) -> Result<String, ArcaferryError> {
    let (emulation, os) = parse_user_agent(user_agent);
    let emulation_opt = wreq_util::EmulationOption::builder()
        .emulation(emulation)
        .emulation_os(os)
        .build();

    let avatar_timeout_secs = std::env::var("ARCAFERRY_AVATAR_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(30)
        .clamp(5, 180);

    let client = wreq::Client::builder()
        .emulation(emulation_opt)
        .timeout(std::time::Duration::from_secs(avatar_timeout_secs))
        .build()
        .map_err(|e| ArcaferryError::NetworkError(e.to_string()))?;

    // Retry a couple times on transient network/timeout errors.
    // Keep retries small to avoid delaying the main response too much.
    let mut last_err: Option<String> = None;
    for attempt in 1..=3 {
        let response = match client.get(url).send().await {
            Ok(r) => r,
            Err(e) => {
                let msg = e.to_string();
                let retryable = msg.to_lowercase().contains("timed out")
                    || msg.to_lowercase().contains("timeout")
                    || msg.to_lowercase().contains("body error")
                    || msg.to_lowercase().contains("connection");
                last_err = Some(msg);
                if retryable && attempt < 3 {
                    tokio::time::sleep(std::time::Duration::from_millis(250 * attempt as u64)).await;
                    continue;
                }
                return Err(ArcaferryError::NetworkError(
                    last_err.unwrap_or_else(|| "avatar request failed".to_string()),
                ));
            }
        };

        if !response.status().is_success() {
            return Err(ArcaferryError::NetworkError(format!(
                "Avatar fetch failed: HTTP {}",
                response.status()
            )));
        }

        match response.bytes().await {
            Ok(bytes) => return Ok(BASE64.encode(&bytes)),
            Err(e) => {
                let msg = e.to_string();
                let retryable = msg.to_lowercase().contains("timed out")
                    || msg.to_lowercase().contains("timeout")
                    || msg.to_lowercase().contains("body error")
                    || msg.to_lowercase().contains("connection");
                last_err = Some(msg);
                if retryable && attempt < 3 {
                    tokio::time::sleep(std::time::Duration::from_millis(250 * attempt as u64)).await;
                    continue;
                }
                return Err(ArcaferryError::NetworkError(
                    last_err.unwrap_or_else(|| "avatar body read failed".to_string()),
                ));
            }
        }
    }

    Err(ArcaferryError::NetworkError(
        last_err.unwrap_or_else(|| "avatar fetch failed".to_string()),
    ))
}

async fn import_handler(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<ImportRequest>,
) -> Result<Json<ImportResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut warnings: Vec<String> = Vec::new();
    let source: String;

    let http_timeout_secs = state.http_timeout_secs_override.unwrap_or(30);

     let (mut quack_info, lorebook_entries, chat_index) = if let Some(json_value) = try_parse_json_object(&request.quack_input) {
         source = "json".to_string();
         warnings.push("使用手动粘贴的 JSON 数据".to_string());

         let info = parse_manual_quack_json(&json_value).map_err(|e| {
             (
                 StatusCode::BAD_REQUEST,
                 Json(ErrorResponse {
                     success: false,
                     error: e.to_string(),
                     error_code: "PARSE_ERROR".to_string(),
                 }),
             )
         })?;

        let mut entries = Vec::new();
        if let Some(ref lb_json) = request.lorebook_json {
            if let Some(lb_value) = try_parse_json_object(lb_json) {
                entries = extract_lorebook_entries_from_json(&lb_value);
            } else {
                warnings.push("世界书 JSON 解析失败".to_string());
            }
        }
        if entries.is_empty() {
            entries = extract_lorebook_entries_from_json(&json_value);
        }

         (info, entries, None)
      } else {
          source = "api".to_string();

        let api_base = effective_api_base(&state, &request.quack_input);
        let url_type = get_url_type(&request.quack_input);

        let id = extract_quack_id(&request.quack_input).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    success: false,
                    error: e.to_string(),
                    error_code: "INVALID_URL".to_string(),
                }),
            )
        })?;

        let cookie_jar = request.cookies.as_ref().and_then(|c| CookieJar::parse(c).ok());
        if request.cookies.is_some() && cookie_jar.is_none() {
            warnings.push("Cookie 解析失败，将尝试无认证请求".to_string());
        }

        let client = QuackClient::new_with_timeout(
            cookie_jar.as_ref(),
            request.bearer_token.as_deref(),
            Some(api_base),
            request.user_agent.as_deref(),
            http_timeout_secs,
        )
        .map_err(|e| {
            let (status, code) = error_to_http(&e);
            (
                status,
                Json(ErrorResponse {
                    success: false,
                    error: e.to_string(),
                    error_code: code.to_string(),
                }),
            )
        })?;

        let api_result = client.fetch_complete_with_type(&id, url_type).await;

         match api_result {
              Ok(result) => result,
              Err(e) => {
                  let (status, code) = error_to_http(&e);
                  let has_user_agent = request
                      .user_agent
                      .as_deref()
                      .is_some_and(|ua| !ua.trim().is_empty());
                  let cookie_jar = request
                      .cookies
                      .as_ref()
                      .and_then(|c| CookieJar::parse(c).ok());
                  let has_cf_clearance =
                      has_cf_clearance_cookie(request.cookies.as_deref(), cookie_jar.as_ref());
                  let error_message =
                      maybe_attach_cloudflare_guidance(&e, code, has_cf_clearance, has_user_agent);
                  return Err((
                      status,
                      Json(ErrorResponse {
                          success: false,
                          error: error_message,
                          error_code: code.to_string(),
                      }),
                  ));
              }
          }
      };

    // Phase 4: optional hidden settings extraction via Python sidecar.
    // Applies to API mode only, and only in full import mode.
    let needs_hidden =
        source == "api" && request.mode == ImportMode::Full && has_empty_hidden_settings(&quack_info);
    debug!(source = %source, mode = ?request.mode, needs_hidden, "import requirements");

    if needs_hidden {
        match &state.browser_capability {
            BrowserCapability::Available => {
                let hidden_labels = get_hidden_attr_labels(&quack_info);
                info!(hidden_labels_count = hidden_labels.len(), "import sidecar hidden extraction check");

                if hidden_labels.is_empty() {
                    warnings.push(
                        "检测到隐藏设定但未找到可提取的隐藏属性标签".to_string(),
                    );
                    warn!("import sidecar skipped: no hidden labels");
                } else {

                // Prefer share URL for browser sidecar.
                 let mut share_url = request.quack_input.clone();
                let api_base = effective_api_base(&state, &request.quack_input);
                let url_type = get_url_type(&request.quack_input);
                if url_type == crate::adapters::quack::QuackUrlType::Dream {
                    if let Ok(id) = extract_quack_id(&request.quack_input) {
                        let cookie_jar = request
                            .cookies
                            .as_ref()
                            .and_then(|c| CookieJar::parse(c).ok());
                        if let Ok(client) = QuackClient::new_with_timeout(
                            cookie_jar.as_ref(),
                            request.bearer_token.as_deref(),
                            Some(api_base),
                            request.user_agent.as_deref(),
                            http_timeout_secs,
                        ) {
                            if let Ok(chat_info) = client.fetch_chat_info_by_index(&id).await {
                                let origin_sid = chat_info
                                    .extra
                                    .get("originSid")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                if !origin_sid.is_empty() {
                                    share_url =
                                        format!("{}/discovery/share/{}", api_base, origin_sid);
                                }
                            }
                        }
                    }
                } else if !share_url.starts_with("http") {
                    if let Ok(id) = extract_quack_id(&share_url) {
                        share_url = format!("{}/discovery/share/{}", api_base, id);
                    }
                }

                let dream_url = chat_index.as_ref().map(|idx| format!("{}/dream/{}", api_base, idx));

                info!("invoking sidecar for hidden settings (import)");
                match extract_hidden_settings_via_sidecar(
                    &share_url,
                    &hidden_labels,
                    crate::browser_sidecar::SidecarInvokeParams {
                        cookies: request.cookies.as_deref(),
                        bearer_token: request.bearer_token.as_deref(),
                        gemini_api_key: request.gemini_api_key.as_deref(),
                        user_agent: request.user_agent.as_deref(),
                        dream_url: dream_url.as_deref(),
                    },
                )
                .await
                {
                    Ok(sidecar) => {
                        let applied = apply_hidden_settings(&mut quack_info, &sidecar.attrs);
                        info!(attrs_count = sidecar.attrs.len(), applied, "import sidecar returned");
                        if applied > 0 {
                            warnings.push("已通过 sidecar 提取隐藏设定".to_string());
                        } else {
                            warnings.push("检测到隐藏设定但 sidecar 未返回有效内容".to_string());
                            warn!("import sidecar returned but nothing applied");
                            if let Some(w) = sidecar_debug_warning(&sidecar) {
                                warnings.push(w);
                            }
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "import sidecar failed");
                        warnings.push(format!("隐藏设定提取失败: {}", e));
                    }
                }
                }
            }
            BrowserCapability::NotInstalled { reason } | BrowserCapability::Error { reason } => {
                info!(reason = %reason, "import sidecar unavailable");
                warnings.push(format!(
                    "隐藏设定提取不可用: {}。基础数据已提取成功。",
                    reason
                ));
            }
        }
    }

    if request.mode == ImportMode::OnlyLorebook {
        if lorebook_entries.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    success: false,
                    error: "该角色没有世界书数据".to_string(),
                    error_code: "PARSE_ERROR".to_string(),
                }),
            ));
        }

        let char_name = quack_info
            .char_list
            .as_ref()
            .and_then(|list| list.first())
            .and_then(|c| c.name.clone())
            .unwrap_or_else(|| quack_info.name.clone());

        let lorebook = map_lorebook(&lorebook_entries, Some(&format!("{}的世界书", char_name)));
        let version_warning = check_version_warning(request.user_agent.as_deref());

        return Ok(Json(ImportResponse {
            success: true,
            card: None,
            lorebook: Some(lorebook),
            png_base64: None,
            avatar_base64: None,
            source,
            warnings,
            version_warning,
        }));
    }

    let card = map_quack_to_v3(&quack_info, &lorebook_entries);

    let avatar_base64 = if let Some(ref picture) = quack_info.picture {
        let avatar_url = if picture.starts_with("http") {
            picture.clone()
        } else {
            format!("{}{}", AVATAR_BASE_URL, picture)
        };
        match fetch_avatar_base64(&avatar_url, request.user_agent.as_deref()).await {
            Ok(b64) => Some(b64),
            Err(_) => {
                warnings.push("封面图片下载失败".to_string());
                None
            }
        }
    } else {
        None
    };

    let png_base64 = if request.output_format == OutputFormat::Png {
        create_card_png(&card, avatar_base64.as_deref())
            .map(|bytes| BASE64.encode(&bytes))
            .map_err(|e| {
                let (status, code) = error_to_http(&e);
                (
                    status,
                    Json(ErrorResponse {
                        success: false,
                        error: e.to_string(),
                        error_code: code.to_string(),
                    }),
                )
            })?
            .into()
    } else {
        None
    };

    let version_warning = check_version_warning(request.user_agent.as_deref());

    Ok(Json(ImportResponse {
        success: true,
        card: Some(card),
        lorebook: None,
        png_base64,
        avatar_base64: if request.output_format == OutputFormat::Json {
            avatar_base64
        } else {
            None
        },
        source,
        warnings,
        version_warning,
    }))
}

async fn preview_handler(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<PreviewRequest>,
) -> Result<Json<PreviewResponse>, (StatusCode, Json<ErrorResponse>)> {
    let http_timeout_secs = state.http_timeout_secs_override.unwrap_or(30);

    let (info, source) = if let Some(json_value) = try_parse_json_object(&request.quack_input) {
        let info = parse_manual_quack_json(&json_value).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    success: false,
                    error: e.to_string(),
                    error_code: "PARSE_ERROR".to_string(),
                }),
            )
        })?;
        (info, "json".to_string())
    } else {
        let api_base = effective_api_base(&state, &request.quack_input);
        let id = extract_quack_id(&request.quack_input).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    success: false,
                    error: e.to_string(),
                    error_code: "INVALID_URL".to_string(),
                }),
            )
        })?;

        let cookie_jar = request.cookies.as_ref().and_then(|c| CookieJar::parse(c).ok());

        let client = QuackClient::new_with_timeout(
            cookie_jar.as_ref(),
            request.bearer_token.as_deref(),
            Some(api_base),
            request.user_agent.as_deref(),
            http_timeout_secs,
        )
        .map_err(|e| {
            let (status, code) = error_to_http(&e);
            (
                status,
                Json(ErrorResponse {
                    success: false,
                    error: e.to_string(),
                    error_code: code.to_string(),
                }),
            )
        })?;

        let info = client.fetch_share_info(&id).await.map_err(|e| {
            let (status, code) = error_to_http(&e);
            let has_user_agent = request
                .user_agent
                .as_deref()
                .is_some_and(|ua| !ua.trim().is_empty());
            let cookie_jar = request
                .cookies
                .as_ref()
                .and_then(|c| CookieJar::parse(c).ok());
            let has_cf_clearance =
                has_cf_clearance_cookie(request.cookies.as_deref(), cookie_jar.as_ref());
            let error_message =
                maybe_attach_cloudflare_guidance(&e, code, has_cf_clearance, has_user_agent);
            (
                status,
                Json(ErrorResponse {
                    success: false,
                    error: error_message,
                    error_code: code.to_string(),
                }),
            )
        })?;

        (info, "api".to_string())
    };

    let mut preview = extract_preview_from_quack(&info);
    preview.source = source;

    Ok(Json(PreviewResponse {
        success: true,
        data: Some(preview),
        error: None,
        error_code: None,
    }))
}

async fn batch_scrape_handler(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<BatchScrapeRequest>,
) -> Result<Json<BatchScrapeResponse>, (StatusCode, Json<ErrorResponse>)> {
    if request.urls.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                error: "No URLs provided".to_string(),
                error_code: "PARSE_ERROR".to_string(),
            }),
        ));
    }

    let concurrency = request.concurrency.clamp(1, 5);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));

    let cookie_jar = request
        .cookies
        .as_ref()
        .and_then(|c| CookieJar::parse(c).ok());
    let cookie_jar = Arc::new(cookie_jar);
    let bearer_token = Arc::new(request.bearer_token.clone());
    let user_agent = Arc::new(request.user_agent.clone());
    let cookies_raw = Arc::new(request.cookies.clone());
    let gemini_api_key = Arc::new(request.gemini_api_key.clone());
    let output_format = request.output_format;
    let api_base_override = Arc::new(state.api_base_override.clone());
    let http_timeout_secs = state.http_timeout_secs_override.unwrap_or(30);
    let browser_capability = Arc::new(state.browser_capability.clone());

    // Sidecar spawns a full browser session per URL, so serialize sidecar calls
    // to avoid spawning N browser instances concurrently.
    let sidecar_mutex = Arc::new(tokio::sync::Mutex::new(()));

    let tasks: Vec<_> = request
        .urls
        .iter()
        .map(|url| {
            let url = url.clone();
            let sem = semaphore.clone();
            let cookies = cookie_jar.clone();
            let token = bearer_token.clone();
            let ua = user_agent.clone();
            let cookies_raw = cookies_raw.clone();
            let gemini_api_key = gemini_api_key.clone();
            let api_base_override = api_base_override.clone();
            let browser_capability = browser_capability.clone();
            let sidecar_mutex = sidecar_mutex.clone();
            async move {
                let _permit = sem.acquire().await.unwrap();
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                scrape_single_url(
                    &url,
                    cookies.as_ref().as_ref(),
                    token.as_deref(),
                    ua.as_deref(),
                    cookies_raw.as_deref(),
                    gemini_api_key.as_deref(),
                    api_base_override.as_deref(),
                    http_timeout_secs,
                    output_format,
                    &browser_capability,
                    &sidecar_mutex,
                )
                .await
            }
        })
        .collect();

    let results: Vec<BatchItemResult> = futures::future::join_all(tasks).await;

    let succeeded = results.iter().filter(|r| r.success).count();
    let failed = results.len() - succeeded;

    let version_warning = check_version_warning(request.user_agent.as_deref());

    Ok(Json(BatchScrapeResponse {
        success: failed == 0,
        total: results.len(),
        succeeded,
        failed,
        results,
        version_warning,
    }))
}

#[allow(clippy::too_many_arguments)]
async fn scrape_single_url(
    url: &str,
    cookies: Option<&CookieJar>,
    bearer_token: Option<&str>,
    user_agent: Option<&str>,
    cookies_raw: Option<&str>,
    gemini_api_key: Option<&str>,
    api_base_override: Option<&str>,
    http_timeout_secs: u64,
    output_format: OutputFormat,
    browser_capability: &BrowserCapability,
    sidecar_mutex: &tokio::sync::Mutex<()>,
) -> BatchItemResult {
    let mut warnings: Vec<String> = Vec::new();
    let api_base = api_base_override.unwrap_or_else(|| get_api_base(url));
    let url_type = get_url_type(url);

    let id = match extract_quack_id(url) {
        Ok(id) => id,
        Err(e) => {
            return BatchItemResult {
                url: url.to_string(),
                success: false,
                card: None,
                avatar_base64: None,
                png_base64: None,
                warnings: Vec::new(),
                error: Some(e.to_string()),
                error_code: Some("INVALID_URL".to_string()),
            }
        }
    };

    let client = match QuackClient::new_with_timeout(
        cookies,
        bearer_token,
        Some(api_base),
        user_agent,
        http_timeout_secs,
    ) {
        Ok(c) => c,
        Err(e) => {
            let (_, code) = error_to_http(&e);
            return BatchItemResult {
                url: url.to_string(),
                success: false,
                card: None,
                avatar_base64: None,
                png_base64: None,
                warnings: Vec::new(),
                error: Some(e.to_string()),
                error_code: Some(code.to_string()),
            }
        }
    };

    match client.fetch_complete_with_type(&id, url_type).await {
        Ok((mut info, lorebook, chat_index)) => {
            let needs_hidden = has_empty_hidden_settings(&info);
            if needs_hidden {
                match browser_capability {
                    BrowserCapability::Available => {
                        let hidden_labels = get_hidden_attr_labels(&info);
                        if hidden_labels.is_empty() {
                            warnings.push(
                                "检测到隐藏设定但未找到可提取的隐藏属性标签".to_string(),
                            );
                        } else {
                            let mut share_url = url.to_string();
                            if !share_url.starts_with("http") {
                                share_url = format!("{}/discovery/share/{}", api_base, id);
                            } else if url_type == crate::adapters::quack::QuackUrlType::Dream {
                                if let Ok(chat_info) = client.fetch_chat_info_by_index(&id).await {
                                    let origin_sid = chat_info
                                        .extra
                                        .get("originSid")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("");
                                    if !origin_sid.is_empty() {
                                        share_url = format!(
                                            "{}/discovery/share/{}",
                                            api_base, origin_sid
                                        );
                                    }
                                }
                            }

                            let dream_url = chat_index.as_ref().map(|idx| format!("{}/dream/{}", api_base, idx));

                            let _sidecar_guard = sidecar_mutex.lock().await;
                            info!(url = %url, "batch: invoking sidecar for hidden settings");
                            match extract_hidden_settings_via_sidecar(
                                &share_url,
                                &hidden_labels,
                                crate::browser_sidecar::SidecarInvokeParams {
                                    cookies: cookies_raw,
                                    bearer_token,
                                    gemini_api_key,
                                    user_agent,
                                    dream_url: dream_url.as_deref(),
                                },
                            )
                            .await
                            {
                                Ok(sidecar) => {
                                    let applied =
                                        apply_hidden_settings(&mut info, &sidecar.attrs);
                                    info!(
                                        attrs_count = sidecar.attrs.len(),
                                        applied,
                                        "batch sidecar returned"
                                    );
                                    if applied > 0 {
                                        warnings
                                            .push("已通过 sidecar 提取隐藏设定".to_string());
                                    } else {
                                        warnings.push(
                                            "检测到隐藏设定但 sidecar 未返回有效内容"
                                                .to_string(),
                                        );
                                        if let Some(w) = sidecar_debug_warning(&sidecar) {
                                            warnings.push(w);
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!(error = %e, url = %url, "batch sidecar failed");
                                    warnings.push(format!("隐藏设定提取失败: {}", e));
                                }
                            }
                        }
                    }
                    BrowserCapability::NotInstalled { reason }
                    | BrowserCapability::Error { reason } => {
                        warnings.push(format!(
                            "隐藏设定提取不可用: {}。基础数据已提取成功。",
                            reason
                        ));
                    }
                }
            }

            let avatar_base64 = if let Some(ref picture) = info.picture {
                let avatar_url = if picture.starts_with("http") {
                    picture.clone()
                } else {
                    format!("{}{}", AVATAR_BASE_URL, picture)
                };
                fetch_avatar_base64(&avatar_url, user_agent).await.ok()
            } else {
                None
            };

            let card = map_quack_to_v3(&info, &lorebook);

            let png_base64 = if output_format == OutputFormat::Png {
                create_card_png(&card, avatar_base64.as_deref())
                    .ok()
                    .map(|bytes| BASE64.encode(&bytes))
            } else {
                None
            };

            BatchItemResult {
                url: url.to_string(),
                success: true,
                card: Some(card),
                avatar_base64: if output_format == OutputFormat::Json {
                    avatar_base64
                } else {
                    None
                },
                png_base64,
                warnings,
                error: None,
                error_code: None,
            }
        }
        Err(e) => {
            let (_, code) = error_to_http(&e);
            let has_user_agent = user_agent.is_some_and(|ua| !ua.trim().is_empty());
            let has_cf_clearance = cookies
                .and_then(|jar| jar.get("cf_clearance"))
                .is_some();
            let error_message =
                maybe_attach_cloudflare_guidance(&e, code, has_cf_clearance, has_user_agent);
            BatchItemResult {
                url: url.to_string(),
                success: false,
                card: None,
                avatar_base64: None,
                png_base64: None,
                warnings: Vec::new(),
                error: Some(error_message),
                error_code: Some(code.to_string()),
            }
        }
    }
}

async fn debug_tls_handler() -> impl IntoResponse {
    let emulation = wreq_util::EmulationOption::builder()
        .emulation(wreq_util::Emulation::Chrome143)
        .emulation_os(wreq_util::EmulationOS::Windows)
        .build();

    let client = match wreq::Client::builder().emulation(emulation).build() {
        Ok(c) => c,
        Err(e) => return Json(serde_json::json!({"error": e.to_string()})),
    };

    match client.get("https://tls.peet.ws/api/all").send().await {
        Ok(resp) => match resp.text().await {
            Ok(body) => Json(
                serde_json::from_str::<serde_json::Value>(&body)
                    .unwrap_or_else(|_| serde_json::json!({"raw": body})),
            ),
            Err(e) => Json(serde_json::json!({"error": e.to_string()})),
        },
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}

pub fn create_router(state: Arc<ServerState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/status", get(status_handler))
        .route("/api/scrape", post(scrape_handler))
        .route("/api/batch", post(batch_scrape_handler))
        .route("/api/import", post(import_handler))
        .route("/api/preview", post(preview_handler))
        .route("/api/debug/tls", get(debug_tls_handler))
        .with_state(state)
        .layer(cors)
}

pub async fn start_server(port: u16) -> Result<(), std::io::Error> {
    let browser_capability = detect_browser_capability();
    let state = Arc::new(ServerState {
        port,
        browser_capability,
        api_base_override: None,
        http_timeout_secs_override: None,
    });
    let app = create_router(state);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;

    println!("Arcaferry HTTP server listening on port {}", port);
    axum::serve(listener, app).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::quack::{get_hidden_attr_labels, QuackAttribute, QuackCharListItem};

    #[test]
    fn test_apply_hidden_settings_updates_multiple_locations() {
        let mut info = QuackCharacterInfo {
            name: "Test".to_string(),
            custom_attrs: Some(serde_json::json!([
                {
                    "label": "TopSecret",
                    "value": "",
                    "isVisible": false,
                    "keepMe": 123
                }
            ])),
            char_list: Some(vec![QuackCharListItem {
                name: Some("Test".to_string()),
                attrs: Some(vec![QuackAttribute {
                    label: Some("A".to_string()),
                    name: None,
                    value: Some("".to_string()),
                    is_visible: Some(false),
                }]),
                advise_attrs: Some(vec![QuackAttribute {
                    label: None,
                    name: Some("B".to_string()),
                    value: Some("".to_string()),
                    is_visible: Some(false),
                }]),
                custom_attrs: Some(vec![QuackAttribute {
                    label: Some("C".to_string()),
                    name: None,
                    value: Some("".to_string()),
                    is_visible: Some(false),
                }]),
                prompt: None,
                picture: None,
                extra: Default::default(),
            }]),
            ..Default::default()
        };

        let extracted = vec![
            QuackAttribute {
                label: Some("TopSecret".to_string()),
                name: None,
                value: Some("V1".to_string()),
                is_visible: Some(false),
            },
            QuackAttribute {
                label: Some("A".to_string()),
                name: None,
                value: Some("V2".to_string()),
                is_visible: Some(false),
            },
            QuackAttribute {
                label: None,
                name: Some("B".to_string()),
                value: Some("V3".to_string()),
                is_visible: Some(false),
            },
            QuackAttribute {
                label: Some("C".to_string()),
                name: None,
                value: Some("V4".to_string()),
                is_visible: Some(false),
            },
        ];

        let applied = apply_hidden_settings(&mut info, &extracted);
        assert_eq!(applied, 4);

        // Top-level customAttrs should keep unknown keys.
        let top = info.custom_attrs.as_ref().unwrap().as_array().unwrap()[0]
            .as_object()
            .unwrap();
        assert_eq!(top.get("value").unwrap().as_str().unwrap(), "V1");
        assert_eq!(top.get("keepMe").unwrap().as_i64().unwrap(), 123);

        let first = info.char_list.as_ref().unwrap().first().unwrap();
        assert_eq!(first.attrs.as_ref().unwrap()[0].value.as_deref(), Some("V2"));
        assert_eq!(
            first.advise_attrs.as_ref().unwrap()[0].value.as_deref(),
            Some("V3")
        );
        assert_eq!(
            first.custom_attrs.as_ref().unwrap()[0].value.as_deref(),
            Some("V4")
        );
    }

    #[test]
    fn test_get_hidden_attr_labels_uses_all_attrs_and_dedupes() {
        let info = QuackCharacterInfo {
            name: "Test".to_string(),
            custom_attrs: Some(serde_json::json!([
                {"label": "X", "value": "", "isVisible": false},
                {"label": "X", "value": "", "isVisible": false}
            ])),
            char_list: Some(vec![QuackCharListItem {
                name: None,
                attrs: Some(vec![QuackAttribute {
                    label: None,
                    name: Some("Y".to_string()),
                    value: Some("".to_string()),
                    is_visible: Some(false),
                }]),
                advise_attrs: None,
                custom_attrs: None,
                prompt: None,
                picture: None,
                extra: Default::default(),
            }]),
            ..Default::default()
        };

        let labels = get_hidden_attr_labels(&info);
        assert_eq!(labels, vec!["X".to_string(), "Y".to_string()]);
    }
}
