//! Platform Adapters
//!
//! Each adapter handles URL parsing and API fetching for a specific platform.

use crate::ccv3::CharacterCardV3;
use crate::error::Result;
use crate::session::Session;
use async_trait::async_trait;

pub mod quack;

pub use quack::{
    extract_quack_id, get_api_base, get_hidden_attr_labels, has_empty_hidden_settings,
    has_placeholder_lorebook, QuackAdapter, QuackApiResponse, QuackAttribute, QuackCharacterInfo,
    QuackClient, QuackGreeting, QuackLorebookEntry,
};

/// Trait for platform-specific adapters.
///
/// Each adapter knows how to:
/// 1. Parse input (URL or ID) to extract a platform-specific identifier
/// 2. Fetch character data from the platform's API
/// 3. Report whether manual verification is required (e.g., Cloudflare challenges)
#[async_trait]
pub trait PlatformAdapter: Send + Sync {
    /// Returns the unique identifier for this platform (e.g., "quack", "chub")
    fn platform_id(&self) -> &'static str;

    /// Parse user input (URL or raw ID) and extract the platform-specific character ID.
    ///
    /// # Arguments
    /// * `input` - User-provided input (URL, raw ID, etc.)
    ///
    /// # Returns
    /// * `Ok(String)` - The extracted character ID
    /// * `Err(ArcaferryError::InvalidUrl)` - If the input cannot be parsed
    fn parse_input(&self, input: &str) -> Result<String>;

    /// Fetch character card data from the platform.
    ///
    /// # Arguments
    /// * `id` - The character ID (as returned by `parse_input`)
    /// * `session` - Optional session containing cookies/tokens for authenticated requests
    ///
    /// # Returns
    /// * `Ok(CharacterCardV3)` - The fetched character card in CCv3 format
    /// * `Err(...)` - Network, auth, or parsing errors
    async fn fetch(&self, id: &str, session: Option<&Session>) -> Result<CharacterCardV3>;

    /// Returns true if this platform typically requires manual verification
    /// (e.g., Cloudflare challenges, captchas, or browser-based authentication).
    fn requires_verification(&self) -> bool;
}
