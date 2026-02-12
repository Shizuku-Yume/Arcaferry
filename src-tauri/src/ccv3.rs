use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LorebookEntry {
    pub keys: Vec<String>,
    #[serde(default)]
    pub secondary_keys: Vec<String>,
    pub content: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub insertion_order: i32,
    #[serde(default)]
    pub case_sensitive: Option<bool>,
    #[serde(default)]
    pub use_regex: bool,
    #[serde(default)]
    pub constant: Option<bool>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub priority: Option<i32>,
    #[serde(default)]
    pub id: Option<Value>, // Can be int or string
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub selective: Option<bool>,
    #[serde(default)]
    pub position: Option<String>, // "before_char" or "after_char"

    // Preserve unknown fields
    #[serde(flatten)]
    pub extensions: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lorebook {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub scan_depth: Option<i32>,
    #[serde(default)]
    pub token_budget: Option<i32>,
    #[serde(default)]
    pub recursive_scanning: Option<bool>,
    #[serde(default)]
    pub entries: Vec<LorebookEntry>,

    #[serde(flatten)]
    pub extensions: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    #[serde(rename = "type")]
    pub asset_type: String,
    pub uri: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub ext: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CharacterCardData {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub personality: String,
    #[serde(default)]
    pub scenario: String,
    #[serde(default)]
    pub first_mes: String,
    #[serde(default)]
    pub mes_example: String,

    #[serde(default)]
    pub alternate_greetings: Vec<String>,
    #[serde(default)]
    pub system_prompt: String,
    #[serde(default)]
    pub post_history_instructions: String,

    #[serde(default)]
    pub character_book: Option<Lorebook>,

    #[serde(default)]
    pub creator: String,
    #[serde(default)]
    pub creator_notes: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub character_version: String,

    #[serde(default)]
    pub creation_date: Option<i64>,
    #[serde(default)]
    pub modification_date: Option<i64>,

    #[serde(default)]
    pub assets: Option<Vec<Asset>>,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub creator_notes_multilingual: Option<HashMap<String, String>>,
    #[serde(default)]
    pub source: Option<Vec<String>>,
    #[serde(default)]
    pub group_only_greetings: Vec<String>,

    #[serde(flatten)]
    pub extensions: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterCardV3 {
    pub spec: String,         // "chara_card_v3"
    pub spec_version: String, // "3.0"
    pub data: CharacterCardData,
}

impl Default for CharacterCardV3 {
    fn default() -> Self {
        Self {
            spec: "chara_card_v3".to_string(),
            spec_version: "3.0".to_string(),
            data: CharacterCardData::default(),
        }
    }
}

impl CharacterCardV3 {
    pub fn new(name: String) -> Self {
        Self {
            spec: "chara_card_v3".to_string(),
            spec_version: "3.0".to_string(),
            data: CharacterCardData {
                name,
                ..Default::default()
            },
        }
    }
}
