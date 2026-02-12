use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "type", content = "details")]
pub enum ArcaferryError {
    // Network errors
    #[error("Network timeout: {0}")]
    Timeout(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Rate limited: retry after {0}s")]
    RateLimited(u64),

    #[error(
        "Cloudflare verification required. Please provide cf_clearance cookie from your browser."
    )]
    CloudflareBlocked,

    #[error("Network error: {0}")]
    NetworkError(String),

    // Parse errors
    #[error("Invalid JSON: {0}")]
    InvalidJson(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    // PNG errors
    #[error("Invalid PNG signature")]
    InvalidPngSignature,

    #[error("PNG chunk error: {0}")]
    PngChunkError(String),

    #[error("No card data found in PNG")]
    NoCardData,

    // Validation errors
    #[error("Validation failed: {0}")]
    ValidationError(String),

    // Session errors
    #[error("Session expired for platform: {0}")]
    SessionExpired(String),

    #[error("No session found for platform: {0}")]
    SessionNotFound(String),

    // Arcamage errors
    #[error("Arcamage connection failed: {0}")]
    ArcamageConnectionFailed(String),

    #[error("Arcamage version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: String, actual: String },

    #[error("Arcamage rejected import: {0}")]
    ImportRejected(String),

    // IO errors
    #[error("IO error: {0}")]
    IoError(String),

    // Browser automation errors
    #[error("Browser error: {0}")]
    BrowserError(String),
}

impl From<wreq::Error> for ArcaferryError {
    fn from(err: wreq::Error) -> Self {
        if err.is_timeout() {
            ArcaferryError::Timeout(err.to_string())
        } else if err.is_connect() {
            ArcaferryError::NetworkError(format!("Connection failed: {}", err))
        } else if let Some(status) = err.status() {
            match status.as_u16() {
                401 | 403 => ArcaferryError::Unauthorized(err.to_string()),
                429 => ArcaferryError::RateLimited(60), // Default retry after 60s
                503 => ArcaferryError::CloudflareBlocked,
                _ => ArcaferryError::NetworkError(err.to_string()),
            }
        } else {
            ArcaferryError::NetworkError(err.to_string())
        }
    }
}

impl From<serde_json::Error> for ArcaferryError {
    fn from(err: serde_json::Error) -> Self {
        ArcaferryError::InvalidJson(err.to_string())
    }
}

impl From<url::ParseError> for ArcaferryError {
    fn from(err: url::ParseError) -> Self {
        ArcaferryError::InvalidUrl(err.to_string())
    }
}

impl From<std::io::Error> for ArcaferryError {
    fn from(err: std::io::Error) -> Self {
        ArcaferryError::IoError(err.to_string())
    }
}

/// Type alias for Result with ArcaferryError
pub type Result<T> = std::result::Result<T, ArcaferryError>;
