//! Quack/Purrly Platform Adapter
//!
//! Handles URL parsing and API fetching for Quack/Purrly character cards.
//! Ported from Arcamage's Python implementation (quack_client.py, purrly_scraper.py).

use crate::adapters::PlatformAdapter;
use crate::ccv3::{CharacterCardData, CharacterCardV3, Lorebook, LorebookEntry};
use crate::cookies::CookieJar;
use crate::error::{ArcaferryError, Result};
use crate::http::HttpClient;
use crate::session::Session;
use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use tracing::{debug, info};
use url::Url;

// API endpoints (from Python quack_client.py and purrly_scraper.py)
pub const PURRLY_API_BASE: &str = "https://purrly.ai";
pub const QUACK_API_BASE: &str = "https://quack.im";
pub const CHARACTER_INFO_PATH: &str = "/api/v1/studioCard/info";
pub const CHAT_INFO_PATH: &str = "/api/v1/user/character/info-by-chat-index";
pub const CHAT_CREATE_PATH: &str = "/api/v1/chats/create";
pub const LOREBOOK_PATH: &str = "/api/v1/chat/getCharacterBooks";

pub const INTERACT_CARD_PATH: &str = "/api/characters/interact-card";
pub const PERSONA_LIST_PATH: &str = "/api/v1/persona/list";
pub const PRESET_LIST_NAME_PATH: &str = "/api/presets/list-name";

pub const AVATAR_BASE_URL: &str = "https://static.purrly.ai/upload_char_avatar/";

// ============================================================================
// API Response Types (matching Python models)
// ============================================================================

/// Generic Quack API response wrapper
#[derive(Debug, Deserialize)]
pub struct QuackApiResponse<T> {
    pub code: i32,
    pub data: Option<T>,
    #[serde(alias = "message")]
    pub msg: Option<String>,
}

#[derive(Debug, Deserialize)]
struct InteractCardResponse {
    pub char: QuackCharacterInfo,
}

#[derive(Debug, Serialize)]
struct InteractCardRequest {
    pub cid: String,
    #[serde(rename = "type")]
    pub card_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatCreateResponse {
    pub cid: String,
    pub index: String,
    #[serde(default)]
    pub characterbooks: Option<Vec<QuackLorebookWrapper>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatCreateRequest {
    pub cid: String,
    #[serde(rename = "type")]
    pub chat_type: String,
    pub name: String,
    pub persona_name: String,
    pub persona_description: Option<String>,
    pub preset: String,
}

/// Character info from /api/v1/studioCard/info
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QuackCharacterInfo {
    #[serde(default)]
    pub sid: Option<Value>,
    #[serde(default)]
    pub id: Option<Value>,
    #[serde(default)]
    pub index: Option<Value>,
    #[serde(default)]
    pub cid: Option<Value>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub personality: Option<String>,
    #[serde(default)]
    pub scenario: Option<String>,
    #[serde(default)]
    pub first_mes: Option<String>,
    #[serde(default)]
    pub mes_example: Option<String>,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub post_history_instructions: Option<String>,
    #[serde(default)]
    pub creator: Option<String>,
    #[serde(default)]
    pub creator_notes: Option<String>,

    // Nested structures
    #[serde(default)]
    pub custom_attrs: Option<Value>,
    #[serde(default)]
    pub greeting: Option<Value>,
    #[serde(default)]
    pub picture: Option<String>,
    #[serde(default)]
    pub char_list: Option<Vec<QuackCharListItem>>,
    #[serde(default)]
    pub chat_info: Option<QuackChatInfo>,
    #[serde(default)]
    pub characterbooks: Option<Vec<QuackLorebookWrapper>>,
    #[serde(default)]
    pub prologue: Option<QuackPrologue>,
    #[serde(default)]
    pub author_name: Option<String>,
    #[serde(default)]
    pub intro: Option<String>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Character list item (charList[0] contains customAttrs)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuackCharListItem {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub attrs: Option<Vec<QuackAttribute>>,
    #[serde(default)]
    pub advise_attrs: Option<Vec<QuackAttribute>>,
    #[serde(default)]
    pub custom_attrs: Option<Vec<QuackAttribute>>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Chat info containing cid and other data
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QuackChatInfo {
    #[serde(default)]
    pub sid: Option<Value>,
    #[serde(default)]
    pub origin_sid: Option<Value>,
    #[serde(default)]
    pub cid: Option<Value>,
    #[serde(default)]
    pub index: Option<Value>,
    #[serde(default)]
    pub char_mes_example: Option<String>,
    #[serde(default)]
    pub char_creator_notes: Option<String>,
    #[serde(default)]
    pub studio_prologue: Option<QuackPrologue>,
    #[serde(default)]
    pub studio_char_list: Option<Vec<QuackCharListItem>>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Prologue structure containing greetings
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuackPrologue {
    #[serde(default)]
    pub greetings: Option<Vec<QuackGreetingItem>>,
}

/// Greeting item in prologue
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuackGreetingItem {
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
}

/// Custom attribute (isVisible determines if it's a hidden setting)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuackAttribute {
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub is_visible: Option<bool>,
}

/// Greeting entry
#[derive(Debug, Clone, Deserialize)]
pub struct QuackGreeting {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
}

/// Lorebook wrapper from API
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuackLorebookWrapper {
    #[serde(default)]
    pub entry_list: Option<Vec<QuackLorebookEntry>>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Lorebook entry from API
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QuackLorebookEntry {
    #[serde(default)]
    pub keys: Option<String>, // comma-separated
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub position: Option<i32>,
    #[serde(default)]
    pub constant: Option<bool>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub insertion_order: Option<i32>,
    #[serde(default)]
    pub case_sensitive: Option<bool>,
    #[serde(default)]
    pub use_regex: Option<bool>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub priority: Option<i32>,
    #[serde(default)]
    pub id: Option<Value>,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub selective: Option<bool>,
    #[serde(default)]
    pub secondary_keys: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

// ============================================================================
// URL Parsing (from Python extract_quack_id)
// ============================================================================

/// Extract character ID from various Quack/Purrly URL formats.
///
/// Supported formats:
/// - `https://purrly.ai/dream/{id}`
/// - `https://purrly.ai/discovery/share/{id}`
/// - `https://quack.im/character/{id}`
/// - `https://quack.im/studio/card/{id}`
/// - `https://purrly.ai/{id}` (single path segment)
/// - Raw ID: alphanumeric string with underscores/hyphens
pub fn extract_quack_id(input: &str) -> Result<String> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err(ArcaferryError::InvalidUrl("Empty input".to_string()));
    }

    // Check if it looks like a URL (contains purrly, quack, or starts with http)
    let is_url = trimmed.to_lowercase().contains("purrly")
        || trimmed.to_lowercase().contains("quack")
        || trimmed.starts_with("http");

    if is_url {
        if let Ok(url) = Url::parse(trimmed) {
            let path_parts: Vec<&str> = url.path().trim_matches('/').split('/').collect();

            // Pattern: /dream/{id}
            for (i, part) in path_parts.iter().enumerate() {
                if *part == "dream" && i + 1 < path_parts.len() {
                    return Ok(path_parts[i + 1].to_string());
                }
            }

            for (i, part) in path_parts.iter().enumerate() {
                if *part == "chat" && i + 1 < path_parts.len() {
                    return Ok(path_parts[i + 1].to_string());
                }
            }

            // Pattern: /discovery/share/{id}
            for (i, part) in path_parts.iter().enumerate() {
                if *part == "share" && i + 1 < path_parts.len() {
                    return Ok(path_parts[i + 1].to_string());
                }
            }

            // Pattern: /character/{id}
            for (i, part) in path_parts.iter().enumerate() {
                if *part == "character" && i + 1 < path_parts.len() {
                    return Ok(path_parts[i + 1].to_string());
                }
            }

            // Pattern: /studio/card/{id}
            for (i, part) in path_parts.iter().enumerate() {
                if *part == "card" && i + 1 < path_parts.len() {
                    return Ok(path_parts[i + 1].to_string());
                }
            }

            // Single path segment: /{id}
            if path_parts.len() == 1 && !path_parts[0].is_empty() {
                return Ok(path_parts[0].to_string());
            }
        }
    }

    // Try as raw ID (alphanumeric + underscore + hyphen)
    let id_regex = Regex::new(r"^[a-zA-Z0-9_-]+$").unwrap();
    if id_regex.is_match(trimmed) {
        return Ok(trimmed.to_string());
    }

    Err(ArcaferryError::InvalidUrl(format!(
        "Cannot extract ID from: {}",
        input
    )))
}

/// URL type for different extraction strategies
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QuackUrlType {
    /// Share/Studio page - ID is the share sid, use directly with studioCard/info
    Share,
    /// Dream/Chat page - ID is the chat index, need to fetch originSid first
    Dream,
    /// Unknown - treat as share ID
    Unknown,
}

/// Determine URL type from input
pub fn get_url_type(input: &str) -> QuackUrlType {
    let lower = input.to_lowercase();

    if lower.contains("/dream/") {
        QuackUrlType::Dream
    } else if lower.contains("/discovery/share/")
        || lower.contains("/studio/card/")
        || lower.contains("/character/")
    {
        QuackUrlType::Share
    } else if lower.contains("purrly") || lower.contains("quack") || lower.starts_with("http") {
        // URL but unknown path - assume share
        QuackUrlType::Share
    } else {
        // Raw ID - assume share
        QuackUrlType::Unknown
    }
}

/// Determine API base URL from input (quack.im/quack.work vs purrly.ai/purrly.art)
pub fn get_api_base(input: &str) -> &'static str {
    let lower = input.to_lowercase();
    // quack.im and quack.work both use quack.im API
    if lower.contains("quack.im") || lower.contains("quack.work") || lower.contains("quack.icu") {
        QUACK_API_BASE
    } else {
        // purrly.ai and purrly.art both use purrly.ai API
        PURRLY_API_BASE
    }
}

// ============================================================================
// Quack Client
// ============================================================================

pub struct QuackClient {
    http: HttpClient,
    api_base: String,
    cookies: Option<CookieJar>,
    bearer_token: Option<String>,
    user_agent: Option<String>,
    authenticated: bool,
}

impl QuackClient {
    pub fn new(
        cookies: Option<&CookieJar>,
        bearer_token: Option<&str>,
        api_base: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<Self> {
        Self::new_with_timeout(cookies, bearer_token, api_base, user_agent, 30)
    }

    pub fn new_with_timeout(
        cookies: Option<&CookieJar>,
        bearer_token: Option<&str>,
        api_base: Option<&str>,
        user_agent: Option<&str>,
        timeout_secs: u64,
    ) -> Result<Self> {
        let http = HttpClient::with_config(cookies, bearer_token, Some(timeout_secs), user_agent)?;
        let authenticated = bearer_token.is_some() || cookies.is_some();
        Ok(Self {
            http,
            api_base: api_base.unwrap_or(PURRLY_API_BASE).to_string(),
            cookies: cookies.cloned(),
            bearer_token: bearer_token.map(|s| s.to_string()),
            user_agent: user_agent.map(|s| s.to_string()),
            authenticated,
        })
    }

    fn guest_param(&self) -> &'static str {
        if self.authenticated { "" } else { "isguest=1&" }
    }

    fn http_with_timeout(&self, timeout_secs: u64) -> Result<HttpClient> {
        HttpClient::with_config(
            self.cookies.as_ref(),
            self.bearer_token.as_deref(),
            Some(timeout_secs),
            self.user_agent.as_deref(),
        )
    }

    async fn get_json_maybe_wrapped<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let text = self.http.get_text(url).await?;

        if let Ok(wrapped) = serde_json::from_str::<QuackApiResponse<T>>(&text) {
            return self.handle_api_response(wrapped);
        }

        serde_json::from_str::<T>(&text).map_err(|e| ArcaferryError::InvalidJson(e.to_string()))
    }

    async fn post_json_maybe_wrapped<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        &self,
        url: &str,
        body: &B,
        timeout_secs: u64,
    ) -> Result<T> {
        let http = self.http_with_timeout(timeout_secs)?;
        let text = http.post_text(url, body).await?;

        if let Ok(wrapped) = serde_json::from_str::<QuackApiResponse<T>>(&text) {
            return self.handle_api_response(wrapped);
        }

        serde_json::from_str::<T>(&text).map_err(|e| ArcaferryError::InvalidJson(e.to_string()))
    }

    pub async fn fetch_character_info(&self, id: &str) -> Result<QuackCharacterInfo> {
        let url = format!("{}{}?{}index={}", self.api_base, CHAT_INFO_PATH, self.guest_param(), id);
        self.get_json_maybe_wrapped(&url).await
    }

    pub async fn fetch_share_info(&self, share_id: &str) -> Result<QuackCharacterInfo> {
        let url = format!(
            "{}{}?{}sid={}",
            self.api_base, CHARACTER_INFO_PATH, self.guest_param(), share_id
        );
        self.get_json_maybe_wrapped(&url).await
    }

    async fn interact_card(&self, share_id: &str) -> Result<QuackCharacterInfo> {
        let url = format!("{}{}?{}", self.api_base, INTERACT_CARD_PATH, self.guest_param().trim_end_matches('&'));
        let req = InteractCardRequest {
            cid: share_id.to_string(),
            card_type: "studio".to_string(),
        };
        let resp: InteractCardResponse = self.post_json_maybe_wrapped(&url, &req, 30).await?;
        Ok(resp.char)
    }

    async fn fetch_persona_default_name(&self) -> Result<String> {
        let url = format!("{}{}?{}", self.api_base, PERSONA_LIST_PATH, self.guest_param().trim_end_matches('&'));
        let data: Value = self.get_json_maybe_wrapped(&url).await?;
        let first = data
            .as_array()
            .and_then(|a| a.first())
            .cloned()
            .unwrap_or(Value::Null);
        let name = first
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("momo")
            .to_string();
        Ok(name)
    }

    async fn fetch_preset_default_name(&self) -> Result<String> {
        let url = format!("{}{}?{}", self.api_base, PRESET_LIST_NAME_PATH, self.guest_param().trim_end_matches('&'));
        let data: Value = self.get_json_maybe_wrapped(&url).await?;
        let first = data
            .as_array()
            .and_then(|a| a.first())
            .cloned()
            .unwrap_or(Value::Null);
        let name = first
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Quack 通用预设")
            .to_string();
        Ok(name)
    }

    async fn create_chat_from_share(
        &self,
        share_id: &str,
    ) -> Result<(ChatCreateResponse, QuackCharacterInfo)> {
        let interacted = self.interact_card(share_id).await?;

        let studio_cid = match interacted.sid.clone().or(interacted.cid.clone()) {
            Some(Value::String(s)) => s,
            Some(Value::Number(n)) => n.to_string(),
            Some(v) => v.to_string(),
            None => String::new(),
        };
        if studio_cid.is_empty() {
            return Err(ArcaferryError::MissingField(
                "studio cid (sid) from interact-card".to_string(),
            ));
        }

        let persona_name = self
            .fetch_persona_default_name()
            .await
            .unwrap_or_else(|_| "momo".to_string());
        let preset = self
            .fetch_preset_default_name()
            .await
            .unwrap_or_else(|_| "Quack 通用预设".to_string());

        let url = format!("{}{}?{}", self.api_base, CHAT_CREATE_PATH, self.guest_param().trim_end_matches('&'));
        let unique_name = format!("ferry_{}", chrono::Utc::now().timestamp_millis());
        let req = ChatCreateRequest {
            cid: studio_cid,
            chat_type: "studio".to_string(),
            name: unique_name,
            persona_name,
            persona_description: None,
            preset,
        };

        let chat: ChatCreateResponse = self.post_json_maybe_wrapped(&url, &req, 60).await?;
        Ok((chat, interacted))
    }

    /// Create a chat session from a share id and return the chat index.
    ///
    /// This is used to obtain a concrete `/dream/{index}` URL for browser-based hidden settings extraction.
    pub async fn create_chat_index_from_share(&self, share_id: &str) -> Result<String> {
        let (chat, _interacted) = self.create_chat_from_share(share_id).await?;
        Ok(chat.index)
    }

    pub async fn fetch_lorebook(
        &self,
        char_index: &str,
        cid: &str,
    ) -> Result<Vec<QuackLorebookEntry>> {
        let url = format!(
            "{}{}?{}index={}&cid={}",
            self.api_base, LOREBOOK_PATH, self.guest_param(), char_index, cid
        );
        let wrappers: Vec<QuackLorebookWrapper> = self.get_json_maybe_wrapped(&url).await?;
        let mut entries = Vec::new();
        for wrapper in wrappers {
            if let Some(entry_list) = wrapper.entry_list {
                entries.extend(entry_list);
            }
        }
        Ok(entries)
    }

    pub async fn fetch_chat_info_by_index(&self, index: &str) -> Result<QuackCharacterInfo> {
        let url = format!(
            "{}{}?{}index={}",
            self.api_base, CHAT_INFO_PATH, self.guest_param(), index
        );
        self.get_json_maybe_wrapped(&url).await
    }

    pub async fn fetch_complete(
        &self,
        id: &str,
    ) -> Result<(QuackCharacterInfo, Vec<QuackLorebookEntry>, Option<String>)> {
        if let Ok(result) = self.fetch_complete_with_type(id, QuackUrlType::Share).await {
            return Ok(result);
        }

        self.fetch_complete_with_type(id, QuackUrlType::Dream).await
    }

    pub async fn fetch_complete_with_type(
        &self,
        id: &str,
        url_type: QuackUrlType,
    ) -> Result<(QuackCharacterInfo, Vec<QuackLorebookEntry>, Option<String>)> {
        fn value_to_string(v: Option<Value>) -> String {
            match v {
                Some(Value::String(s)) => s,
                Some(Value::Number(n)) => n.to_string(),
                Some(v) => v.to_string(),
                None => String::new(),
            }
        }

        fn flatten_lorebook_wrappers(wrappers: &[QuackLorebookWrapper]) -> Vec<QuackLorebookEntry> {
            let mut entries = Vec::new();
            for wrapper in wrappers {
                if let Some(ref entry_list) = wrapper.entry_list {
                    entries.extend(entry_list.clone());
                }
            }
            entries
        }

        let mut lorebook_entries: Vec<QuackLorebookEntry> = Vec::new();

        let (info, share_id, mut index, mut cid) = match url_type {
            QuackUrlType::Dream => {
                let chat_character = self.fetch_chat_info_by_index(id).await?;
                let origin_sid = chat_character
                    .extra
                    .get("originSid")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default();

                if origin_sid.is_empty() {
                    return Err(ArcaferryError::MissingField(
                        "originSid in chat info".to_string(),
                    ));
                }

                if let Some(ref books) = chat_character.characterbooks {
                    lorebook_entries = flatten_lorebook_wrappers(books);
                    if is_placeholder_entries(&lorebook_entries) {
                        lorebook_entries.clear();
                    }
                }

                let cid =
                    value_to_string(chat_character.sid.clone().or(chat_character.cid.clone()));
                let info = self.fetch_share_info(&origin_sid).await?;
                (info, origin_sid, id.to_string(), cid)
            }
            QuackUrlType::Share | QuackUrlType::Unknown => {
                let info = self.fetch_share_info(id).await?;
                (info, id.to_string(), String::new(), String::new())
            }
        };

        let needs_lorebook = has_placeholder_lorebook(&info);
        let needs_hidden = has_empty_hidden_settings(&info);

        debug!(
            needs_lorebook = needs_lorebook,
            needs_hidden = needs_hidden,
            "Checked extraction requirements"
        );

        if (needs_lorebook || needs_hidden) && index.is_empty() && cid.is_empty() {
            info!("Creating chat session to obtain cid/index for extraction");
            let (chat, _interacted) = self.create_chat_from_share(&share_id).await?;
            index = chat.index;
            cid = chat.cid;
            debug!(cid = %cid, index = %index, "Chat session created");

            if let Some(books) = chat.characterbooks {
                lorebook_entries = flatten_lorebook_wrappers(&books);
                if is_placeholder_entries(&lorebook_entries) {
                    lorebook_entries.clear();
                }
            }

            if needs_lorebook && lorebook_entries.is_empty() {
                if let Ok(chat_character) = self.fetch_chat_info_by_index(&index).await {
                    if let Some(ref books) = chat_character.characterbooks {
                        lorebook_entries = flatten_lorebook_wrappers(books);
                        if is_placeholder_entries(&lorebook_entries) {
                            lorebook_entries.clear();
                        }
                    }
                }
            }
        }

        if needs_lorebook && lorebook_entries.is_empty() && !cid.is_empty() && !index.is_empty() {
            let fetched = self.fetch_lorebook(&index, &cid).await.unwrap_or_default();
            if !is_placeholder_entries(&fetched) {
                lorebook_entries = fetched;
            }
        }

        if needs_hidden {
            debug!(
                cid_empty = cid.is_empty(),
                index_empty = index.is_empty(),
                "Hidden settings detected but API-based extraction has been removed"
            );
        }

        // Return the chat index if we have one (for dream URL construction by caller).
        let chat_index = if !index.is_empty() {
            Some(index)
        } else {
            None
        };

        Ok((info, lorebook_entries, chat_index))
    }

    fn handle_api_response<T>(&self, response: QuackApiResponse<T>) -> Result<T> {
        // Check API-level error code
        if response.code != 0 {
            let msg = response
                .msg
                .unwrap_or_else(|| "Unknown API error".to_string());
            if response.code == 401 || msg.to_lowercase().contains("auth") {
                return Err(ArcaferryError::Unauthorized(format!(
                    "Cookie Invalid - {}",
                    msg
                )));
            }
            return Err(ArcaferryError::NetworkError(format!(
                "Quack API error (code {}): {}",
                response.code, msg
            )));
        }

        response
            .data
            .ok_or_else(|| ArcaferryError::MissingField("character data".to_string()))
    }

    /// Build avatar URL from picture path
    pub fn build_avatar_url(picture: &str) -> String {
        if picture.starts_with("http") {
            picture.to_string()
        } else {
            format!("{}{}", AVATAR_BASE_URL, picture)
        }
    }
}

// ============================================================================
// Helper functions for checking character data state
// ============================================================================

fn is_placeholder_entries(entries: &[QuackLorebookEntry]) -> bool {
    entries.iter().any(|e| {
        let content = e.content.as_deref().unwrap_or("");
        content.is_empty() || content == "_" || content == "-" || content.len() <= 1
    })
}

/// Check if character has placeholder lorebook entries that need browser extraction.
pub fn has_placeholder_lorebook(info: &QuackCharacterInfo) -> bool {
    let books = match info.characterbooks.as_ref() {
        Some(b) => b,
        None => return false,
    };
    for book in books {
        let entries = match book.entry_list.as_ref() {
            Some(e) => e,
            None => continue,
        };
        if is_placeholder_entries(entries) {
            return true;
        }
    }
    false
}

/// Check if character has empty hidden settings that need extraction.
pub fn has_empty_hidden_settings(info: &QuackCharacterInfo) -> bool {
    let attrs = collect_all_attrs(info);
    attrs
        .iter()
        .any(|a| a.is_visible == Some(false) && a.value.as_deref().unwrap_or("").is_empty())
}

/// Get labels of hidden attributes that have empty values.
pub fn get_hidden_attr_labels(info: &QuackCharacterInfo) -> Vec<String> {
    // IMPORTANT:
    // - `has_empty_hidden_settings()` uses `collect_all_attrs()` (covers multiple locations).
    // - If we only look at `charList[0].customAttrs`, we can end up in a state where
    //   `needs_hidden=true` but `hidden_labels` is empty → sidecar skipped.
    //
    // Therefore: derive labels from the same unified attribute view, and fall back to `name`
    // when `label` is missing.
    let all_attrs = collect_all_attrs(info);
    let mut seen: HashSet<String> = HashSet::new();
    let mut labels: Vec<String> = Vec::new();

    for attr in all_attrs {
        if attr.is_visible != Some(false) {
            continue;
        }
        let value = attr.value.as_deref().unwrap_or("").trim();
        if !value.is_empty() {
            continue;
        }

        let key = attr
            .label
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .or_else(|| {
                attr.name
                    .as_deref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
            });

        if let Some(k) = key {
            if seen.insert(k.clone()) {
                labels.push(k);
            }
        }
    }

    labels
}

// ============================================================================
// Mapper: QuackCharacterInfo -> CharacterCardV3
// (Ported from Arcamage's Python quack_mapper.py)
// ============================================================================

/// Format attributes as [Label: Value] blocks
///
/// This is a HARD CONSTRAINT - attrs MUST be formatted exactly as [Label: Value].
pub fn format_attrs(attrs: &[QuackAttribute], visible_only: bool) -> String {
    attrs
        .iter()
        .filter(|a| !visible_only || a.is_visible.unwrap_or(true))
        .filter_map(|a| {
            let label = a.label.as_ref().or(a.name.as_ref())?;
            let value = a.value.as_ref()?;
            if !label.is_empty() && !value.is_empty() {
                Some(format!("[{}: {}]", label, value))
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format hidden attributes (is_visible = false)
pub fn format_hidden_attrs(attrs: &[QuackAttribute]) -> String {
    attrs
        .iter()
        .filter(|a| !a.is_visible.unwrap_or(true))
        .filter_map(|a| {
            let label = a.label.as_ref().or(a.name.as_ref())?;
            let value = a.value.as_ref()?;
            if !label.is_empty() && !value.is_empty() {
                Some(format!("[{}: {}]", label, value))
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extract personality from attributes
pub fn extract_personality(attrs: &[QuackAttribute]) -> String {
    attrs
        .iter()
        .find(|a| {
            let label = a.label.as_ref().or(a.name.as_ref());
            label
                .map(|l| l.to_lowercase() == "personality")
                .unwrap_or(false)
        })
        .and_then(|a| a.value.clone())
        .unwrap_or_default()
}

/// Extract greetings from Quack format
///
/// Returns (first_mes, alternate_greetings)
/// CRITICAL: Preserve HTML exactly - byte-level fidelity
pub fn extract_greetings(greetings: &Value) -> (String, Vec<String>) {
    let arr = match greetings.as_array() {
        Some(a) => a,
        None => return (String::new(), Vec::new()),
    };

    let greeting_values: Vec<String> = arr
        .iter()
        .filter_map(|g| {
            g.get("value")
                .and_then(|v| v.as_str())
                .or_else(|| g.get("content").and_then(|v| v.as_str()))
                .or_else(|| g.get("text").and_then(|v| v.as_str()))
                .map(|s| s.to_string())
        })
        .filter(|s| !s.is_empty())
        .collect();

    if greeting_values.is_empty() {
        return (String::new(), Vec::new());
    }

    let first = greeting_values[0].clone();
    let alts = greeting_values[1..].to_vec();

    (first, alts)
}

fn extract_greetings_from_prologue(prologue: &QuackPrologue) -> (String, Vec<String>) {
    let greetings = match &prologue.greetings {
        Some(g) => g,
        None => return (String::new(), Vec::new()),
    };

    let greeting_values: Vec<String> = greetings
        .iter()
        .filter_map(|g| {
            g.value
                .as_ref()
                .or(g.content.as_ref())
                .or(g.text.as_ref())
                .cloned()
        })
        .filter(|s| !s.is_empty())
        .collect();

    if greeting_values.is_empty() {
        return (String::new(), Vec::new());
    }

    let first = greeting_values[0].clone();
    let alts = greeting_values[1..].to_vec();

    (first, alts)
}

/// Parse comma-separated keys string into Vec
fn parse_keys(keys_str: Option<&String>) -> Vec<String> {
    keys_str
        .map(|k| {
            k.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Map Quack lorebook entry to CCv3 format
///
/// HARD CONSTRAINTS:
/// - constant=true entries can have empty keys (must not be dropped)
/// - selective is dynamically calculated based on secondary_keys presence
/// - Position mapping: 0 → "before_char", 1 → "after_char"
pub fn map_lorebook_entry(entry: &QuackLorebookEntry, index: i32) -> LorebookEntry {
    // Parse comma-separated keys
    let mut keys = parse_keys(entry.keys.as_ref());

    // Check constant flag
    let constant = entry.constant.unwrap_or(false);

    // HARD CONSTRAINT: If keys are empty and NOT constant, fall back to name
    if keys.is_empty() && !constant {
        if let Some(ref name) = entry.name {
            if !name.is_empty() {
                keys = vec![name.clone()];
            }
        }
    }
    // HARD CONSTRAINT: constant=true entries can have empty keys - DO NOT DROP

    // Parse secondary keys
    let secondary_keys = parse_keys(entry.secondary_keys.as_ref());

    // CRITICAL: selective = true ONLY when secondary_keys is not empty
    let selective = !secondary_keys.is_empty();

    // CRITICAL: Position mapping - 0 → "before_char", 1 → "after_char"
    let position = match entry.position {
        Some(0) => Some("before_char".to_string()),
        Some(1) => Some("after_char".to_string()),
        _ => Some("before_char".to_string()), // Default to before_char
    };

    LorebookEntry {
        keys,
        secondary_keys,
        content: entry.content.clone().unwrap_or_default(),
        enabled: entry.enabled.unwrap_or(true),
        insertion_order: index + 1,
        case_sensitive: Some(entry.case_sensitive.unwrap_or(false)),
        use_regex: entry.use_regex.unwrap_or(false),
        constant: Some(constant),
        name: entry.name.clone(),
        priority: Some(entry.priority.unwrap_or(10)),
        id: Some(Value::Number((index + 1).into())),
        comment: entry.comment.clone(),
        selective: Some(selective),
        position,
        extensions: entry.extra.clone(),
    }
}

/// Map Quack lorebook to CCv3 format
pub fn map_lorebook(entries: &[QuackLorebookEntry], book_name: Option<&str>) -> Lorebook {
    Lorebook {
        name: book_name.unwrap_or("Quack Lore").to_string(),
        description: String::new(),
        scan_depth: Some(50),
        token_budget: Some(500),
        recursive_scanning: Some(false),
        entries: entries
            .iter()
            .enumerate()
            .map(|(i, e)| map_lorebook_entry(e, i as i32))
            .collect(),
        extensions: HashMap::new(),
    }
}

/// Collect all attributes from character info
pub fn collect_all_attrs(info: &QuackCharacterInfo) -> Vec<QuackAttribute> {
    let mut all_attrs = Vec::new();

    fn parse_attrs(value: &Value) -> Vec<QuackAttribute> {
        match value.as_array() {
            Some(arr) => arr
                .iter()
                .filter_map(|v| serde_json::from_value::<QuackAttribute>(v.clone()).ok())
                .collect(),
            None => Vec::new(),
        }
    }

    if let Some(ref attrs) = info.custom_attrs {
        all_attrs.extend(parse_attrs(attrs));
    }

    if let Some(ref char_list) = info.char_list {
        if let Some(first_char) = char_list.first() {
            if let Some(ref attrs) = first_char.attrs {
                all_attrs.extend(attrs.clone());
            }
            if let Some(ref attrs) = first_char.advise_attrs {
                all_attrs.extend(attrs.clone());
            }
            if let Some(ref attrs) = first_char.custom_attrs {
                all_attrs.extend(attrs.clone());
            }
        }
    }

    all_attrs
}

/// Extract tags - ALWAYS include "QuackAI" as first tag
fn extract_tags(info: &QuackCharacterInfo) -> Vec<String> {
    let mut tags: Vec<String> = info
        .extra
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    // Force include QuackAI tag as first
    if !tags.iter().any(|t| t == "QuackAI") {
        tags.insert(0, "QuackAI".to_string());
    }

    tags
}

/// Main mapping function: Quack info + lorebook → CCv3
///
/// This is the main mapping function that enforces all hard constraints:
/// - HTML greeting preservation (byte-level)
/// - attrs formatted as [Label: Value]
/// - constant=true lore entries preserved even with empty keys
/// - selective dynamically calculated from secondary_keys
pub fn map_quack_to_v3(
    info: &QuackCharacterInfo,
    lorebook_entries: &[QuackLorebookEntry],
) -> CharacterCardV3 {
    let first_char = info.char_list.as_ref().and_then(|list| list.first());

    let name = first_char
        .and_then(|c| c.name.clone())
        .unwrap_or_else(|| info.name.clone());

    let all_attrs = collect_all_attrs(info);

    let description = format_attrs(&all_attrs, true);

    let personality = info
        .personality
        .clone()
        .unwrap_or_else(|| extract_personality(&all_attrs));

    let (mut first_mes, mut alternate_greetings) = info
        .prologue
        .as_ref()
        .map(extract_greetings_from_prologue)
        .unwrap_or_default();

    if first_mes.is_empty() {
        let (fm, alts) = info
            .greeting
            .as_ref()
            .map(extract_greetings)
            .unwrap_or_default();
        first_mes = fm;
        if alternate_greetings.is_empty() {
            alternate_greetings = alts;
        }
    }

    if first_mes.is_empty() {
        first_mes = info.first_mes.clone().unwrap_or_default();
    }

    if let Some(ref chat_info) = info.chat_info {
        if let Some(ref studio_prologue) = chat_info.studio_prologue {
            let (_, studio_alts) = extract_greetings_from_prologue(studio_prologue);
            let existing: std::collections::HashSet<_> =
                alternate_greetings.iter().cloned().collect();
            for g in studio_alts {
                if !g.is_empty() && !existing.contains(&g) && g != first_mes {
                    alternate_greetings.push(g);
                }
            }
        }
    }

    let tags = extract_tags(info);

    let character_book = if lorebook_entries.is_empty() {
        info.characterbooks.as_ref().and_then(|books| {
            let all_entries: Vec<QuackLorebookEntry> = books
                .iter()
                .filter_map(|b| b.entry_list.clone())
                .flatten()
                .collect();
            if all_entries.is_empty() {
                None
            } else {
                Some(map_lorebook(
                    &all_entries,
                    Some(&format!("{}的世界书", name)),
                ))
            }
        })
    } else {
        Some(map_lorebook(
            lorebook_entries,
            Some(&format!("{}的世界书", name)),
        ))
    };

    let mut system_prompt = first_char
        .and_then(|c| c.prompt.clone())
        .or_else(|| info.system_prompt.clone())
        .unwrap_or_default();

    let hidden_attrs_block = format_hidden_attrs(&all_attrs);
    if !hidden_attrs_block.is_empty() {
        if !system_prompt.is_empty() {
            system_prompt = format!("{}\n\n{}", system_prompt, hidden_attrs_block);
        } else {
            system_prompt = hidden_attrs_block;
        }
    }

    let mes_example = info
        .chat_info
        .as_ref()
        .and_then(|ci| ci.char_mes_example.clone())
        .or_else(|| info.mes_example.clone())
        .unwrap_or_default();

    let creator = info
        .author_name
        .clone()
        .or_else(|| info.creator.clone())
        .unwrap_or_default();

    let creator_notes = info
        .chat_info
        .as_ref()
        .and_then(|ci| ci.char_creator_notes.clone())
        .or_else(|| info.creator_notes.clone())
        .or_else(|| info.intro.clone())
        .unwrap_or_default();

    let current_time = chrono::Utc::now().timestamp();

    CharacterCardV3 {
        spec: "chara_card_v3".to_string(),
        spec_version: "3.0".to_string(),
        data: CharacterCardData {
            name,
            description,
            personality,
            scenario: info.scenario.clone().unwrap_or_default(),
            first_mes,
            mes_example,
            alternate_greetings,
            system_prompt,
            post_history_instructions: info.post_history_instructions.clone().unwrap_or_default(),
            character_book,
            creator,
            creator_notes,
            tags,
            character_version: "1.0".to_string(),
            creation_date: Some(current_time),
            modification_date: Some(current_time),
            ..Default::default()
        },
    }
}

impl QuackCharacterInfo {
    pub fn to_ccv3(&self, lorebook_entries: &[QuackLorebookEntry]) -> CharacterCardV3 {
        map_quack_to_v3(self, lorebook_entries)
    }
}

// ============================================================================
// Platform Adapter Implementation
// ============================================================================

pub struct QuackAdapter {
    default_api_base: String,
}

impl QuackAdapter {
    pub fn new() -> Self {
        Self {
            default_api_base: PURRLY_API_BASE.to_string(),
        }
    }
}

impl Default for QuackAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlatformAdapter for QuackAdapter {
    fn platform_id(&self) -> &'static str {
        "quack"
    }

    fn parse_input(&self, input: &str) -> Result<String> {
        extract_quack_id(input)
    }

    async fn fetch(&self, id: &str, session: Option<&Session>) -> Result<CharacterCardV3> {
        // Extract cookies and bearer token from session
        let cookie_jar = session.and_then(|s| s.get_cookie_jar().ok()).flatten();
        let bearer_token = session.and_then(|s| s.bearer_token.as_deref());

        // Determine API base from the original input (stored in session metadata or use default)
        let api_base = &self.default_api_base;

        let client = QuackClient::new(cookie_jar.as_ref(), bearer_token, Some(api_base), None)?;

        let (info, lorebook, _chat_index) = client.fetch_complete(id).await?;
        Ok(info.to_ccv3(&lorebook))
    }

    fn requires_verification(&self) -> bool {
        true
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_quack_id_raw() {
        assert_eq!(extract_quack_id("abc123").unwrap(), "abc123");
        assert_eq!(extract_quack_id("ABC_123-xyz").unwrap(), "ABC_123-xyz");
        assert_eq!(
            extract_quack_id("-SEgbwsYFEBj3Q2yFRnBZ").unwrap(),
            "-SEgbwsYFEBj3Q2yFRnBZ"
        );
    }

    #[test]
    fn test_extract_quack_id_purrly_dream() {
        assert_eq!(
            extract_quack_id("https://purrly.ai/dream/abc123").unwrap(),
            "abc123"
        );
        assert_eq!(
            extract_quack_id("https://purrly.ai/dream/-SEgbwsYFEBj3Q2yFRnBZ").unwrap(),
            "-SEgbwsYFEBj3Q2yFRnBZ"
        );
    }

    #[test]
    fn test_extract_quack_id_purrly_share() {
        assert_eq!(
            extract_quack_id("https://purrly.ai/discovery/share/-SEgbwsYFEBj3Q2yFRnBZ").unwrap(),
            "-SEgbwsYFEBj3Q2yFRnBZ"
        );
        assert_eq!(
            extract_quack_id("https://purrly.ai/discovery/share/-SEgbwsYFEBj3Q2yFRnBZ?type=studio")
                .unwrap(),
            "-SEgbwsYFEBj3Q2yFRnBZ"
        );
    }

    #[test]
    fn test_extract_quack_id_quack_character() {
        assert_eq!(
            extract_quack_id("https://quack.im/character/abc123").unwrap(),
            "abc123"
        );
    }

    #[test]
    fn test_extract_quack_id_quack_studio() {
        assert_eq!(
            extract_quack_id("https://quack.im/studio/card/abc123").unwrap(),
            "abc123"
        );
        assert_eq!(
            extract_quack_id("https://quack.icu/studio/card/abc123").unwrap(),
            "abc123"
        );
        assert_eq!(
            extract_quack_id("https://quack.work/studio/card/abc123").unwrap(),
            "abc123"
        );
    }

    #[test]
    fn test_extract_quack_id_single_path() {
        assert_eq!(
            extract_quack_id("https://purrly.ai/abc123").unwrap(),
            "abc123"
        );
    }

    #[test]
    fn test_extract_quack_id_invalid() {
        assert!(extract_quack_id("").is_err());
        assert!(extract_quack_id("   ").is_err());
        assert!(extract_quack_id("https://example.com/").is_err());
    }

    #[test]
    fn test_get_api_base() {
        assert_eq!(get_api_base("https://quack.im/dream/abc"), QUACK_API_BASE);
        assert_eq!(get_api_base("https://quack.icu/dream/abc"), QUACK_API_BASE);
        assert_eq!(get_api_base("https://quack.work/dream/abc"), QUACK_API_BASE);
        assert_eq!(get_api_base("https://purrly.ai/dream/abc"), PURRLY_API_BASE);
        assert_eq!(
            get_api_base("https://purrly.art/dream/abc"),
            PURRLY_API_BASE
        );
        assert_eq!(get_api_base("abc123"), PURRLY_API_BASE);
    }

    #[test]
    fn test_get_url_type() {
        assert_eq!(
            get_url_type("https://purrly.ai/dream/abc"),
            QuackUrlType::Dream
        );
        assert_eq!(
            get_url_type("https://quack.im/discovery/share/abc?type=studio"),
            QuackUrlType::Share
        );
        assert_eq!(
            get_url_type("https://quack.icu/studio/card/abc"),
            QuackUrlType::Share
        );
        assert_eq!(
            get_url_type("https://quack.work/character/abc"),
            QuackUrlType::Share
        );
        assert_eq!(get_url_type("abc123"), QuackUrlType::Unknown);
    }

    #[test]
    fn test_build_avatar_url() {
        assert_eq!(
            QuackClient::build_avatar_url("avatar.png"),
            "https://static.purrly.ai/upload_char_avatar/avatar.png"
        );
        assert_eq!(
            QuackClient::build_avatar_url("https://example.com/avatar.png"),
            "https://example.com/avatar.png"
        );
    }

    #[test]
    fn test_adapter_parse_input() {
        let adapter = QuackAdapter::new();
        assert_eq!(adapter.parse_input("abc123").unwrap(), "abc123");
        assert_eq!(
            adapter
                .parse_input("https://purrly.ai/dream/abc123")
                .unwrap(),
            "abc123"
        );
        assert_eq!(
            adapter
                .parse_input("https://quack.im/studio/card/abc123")
                .unwrap(),
            "abc123"
        );
    }

    #[test]
    fn test_format_attrs() {
        let attrs = vec![
            QuackAttribute {
                label: Some("Age".to_string()),
                name: None,
                value: Some("25".to_string()),
                is_visible: Some(true),
            },
            QuackAttribute {
                label: Some("Hidden".to_string()),
                name: None,
                value: Some("secret".to_string()),
                is_visible: Some(false),
            },
            QuackAttribute {
                label: Some("Height".to_string()),
                name: None,
                value: Some("170cm".to_string()),
                is_visible: None,
            },
        ];

        let result = format_attrs(&attrs, true);
        assert_eq!(result, "[Age: 25]\n[Height: 170cm]");

        let result_all = format_attrs(&attrs, false);
        assert_eq!(result_all, "[Age: 25]\n[Hidden: secret]\n[Height: 170cm]");
    }

    #[test]
    fn test_format_hidden_attrs() {
        let attrs = vec![
            QuackAttribute {
                label: Some("Age".to_string()),
                name: None,
                value: Some("25".to_string()),
                is_visible: Some(true),
            },
            QuackAttribute {
                label: Some("Hidden".to_string()),
                name: None,
                value: Some("secret".to_string()),
                is_visible: Some(false),
            },
        ];

        let result = format_hidden_attrs(&attrs);
        assert_eq!(result, "[Hidden: secret]");
    }

    #[test]
    fn test_extract_personality() {
        let attrs = vec![
            QuackAttribute {
                label: Some("Age".to_string()),
                name: None,
                value: Some("25".to_string()),
                is_visible: Some(true),
            },
            QuackAttribute {
                label: Some("Personality".to_string()),
                name: None,
                value: Some("cheerful and kind".to_string()),
                is_visible: Some(true),
            },
        ];

        assert_eq!(extract_personality(&attrs), "cheerful and kind");
    }

    #[test]
    fn test_map_lorebook_entry_selective() {
        let entry_with_secondary = QuackLorebookEntry {
            name: Some("Test Entry".to_string()),
            keys: Some("key1, key2".to_string()),
            secondary_keys: Some("sec1, sec2".to_string()),
            content: Some("Test content".to_string()),
            ..Default::default()
        };

        let mapped = map_lorebook_entry(&entry_with_secondary, 0);
        assert_eq!(mapped.selective, Some(true));
        assert_eq!(mapped.keys, vec!["key1", "key2"]);
        assert_eq!(mapped.secondary_keys, vec!["sec1", "sec2"]);

        let entry_without_secondary = QuackLorebookEntry {
            name: Some("Test Entry".to_string()),
            keys: Some("key1, key2".to_string()),
            secondary_keys: None,
            content: Some("Test content".to_string()),
            ..Default::default()
        };

        let mapped = map_lorebook_entry(&entry_without_secondary, 0);
        assert_eq!(mapped.selective, Some(false));
    }

    #[test]
    fn test_map_lorebook_entry_constant_empty_keys() {
        let entry = QuackLorebookEntry {
            name: Some("Constant Entry".to_string()),
            keys: None,
            content: Some("Always included content".to_string()),
            constant: Some(true),
            ..Default::default()
        };

        let mapped = map_lorebook_entry(&entry, 0);
        assert_eq!(mapped.constant, Some(true));
        assert!(mapped.keys.is_empty());
        assert_eq!(mapped.content, "Always included content");
    }

    #[test]
    fn test_map_lorebook_entry_position() {
        let entry_before = QuackLorebookEntry {
            position: Some(0),
            content: Some("Before".to_string()),
            ..Default::default()
        };
        let mapped = map_lorebook_entry(&entry_before, 0);
        assert_eq!(mapped.position, Some("before_char".to_string()));

        let entry_after = QuackLorebookEntry {
            position: Some(1),
            content: Some("After".to_string()),
            ..Default::default()
        };
        let mapped = map_lorebook_entry(&entry_after, 0);
        assert_eq!(mapped.position, Some("after_char".to_string()));
    }

    #[test]
    fn test_extract_greetings_html_preservation() {
        let html_content = "<p>Hello <strong>world</strong>!</p>";
        let greetings = serde_json::json!([{ "content": html_content }]);

        let (first_mes, _) = extract_greetings(&greetings);
        assert_eq!(first_mes, html_content);
    }

    #[test]
    fn test_map_quack_to_v3_basic() {
        let attrs = vec![QuackAttribute {
            label: Some("Age".to_string()),
            name: None,
            value: Some("25".to_string()),
            is_visible: Some(true),
        }];

        let info = QuackCharacterInfo {
            name: "Test Character".to_string(),
            custom_attrs: Some(serde_json::to_value(attrs).unwrap()),
            ..Default::default()
        };

        let card = map_quack_to_v3(&info, &[]);
        assert_eq!(card.data.name, "Test Character");
        assert!(card.data.tags.contains(&"QuackAI".to_string()));
        assert_eq!(card.spec, "chara_card_v3");
        assert_eq!(card.spec_version, "3.0");
    }

    #[test]
    fn test_map_quack_to_v3_tags_always_has_quackai() {
        let info = QuackCharacterInfo {
            name: "Test".to_string(),
            ..Default::default()
        };

        let card = map_quack_to_v3(&info, &[]);
        assert_eq!(card.data.tags[0], "QuackAI");
    }
}
