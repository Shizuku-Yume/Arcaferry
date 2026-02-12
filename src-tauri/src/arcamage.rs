use crate::ccv3::CharacterCardV3;
use crate::error::{ArcaferryError, Result};
use serde::{Deserialize, Serialize};
use wreq::{multipart, Client};
use wreq_util::Emulation;

const ARCAFERRY_VERSION: &str = "0.1.0";
const DEFAULT_FORGE_URL: &str = "http://localhost:8000";

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportResponse {
    pub success: bool,
    pub card_id: Option<String>,
    pub message: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Clone)]
pub struct ArcamageClient {
    base_url: String,
    api_token: Option<String>,
    client: Client,
}

impl ArcamageClient {
    pub fn new(base_url: Option<&str>, api_token: Option<&str>) -> Result<Self> {
        let emulation = Emulation::Chrome143;

        let client = Client::builder()
            .emulation(emulation)
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| ArcaferryError::NetworkError(e.to_string()))?;

        Ok(Self {
            base_url: base_url
                .unwrap_or(DEFAULT_FORGE_URL)
                .trim_end_matches('/')
                .to_string(),
            api_token: api_token.map(|s| s.to_string()),
            client,
        })
    }

    /// Send CCv3 JSON to Arcamage
    pub async fn send_json(&self, card: &CharacterCardV3) -> Result<ImportResponse> {
        let url = format!("{}/api/import/remote", self.base_url);

        let mut request = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-Arcaferry-Version", ARCAFERRY_VERSION)
            .json(card);

        if let Some(ref token) = self.api_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let response = request.send().await.map_err(|e| {
            if e.is_connect() {
                ArcaferryError::ArcamageConnectionFailed(e.to_string())
            } else {
                ArcaferryError::NetworkError(e.to_string())
            }
        })?;

        self.handle_response(response).await
    }

    /// Send PNG file to Arcamage
    pub async fn send_png(&self, png_data: &[u8], filename: &str) -> Result<ImportResponse> {
        let url = format!("{}/api/import/remote", self.base_url);

        let part = multipart::Part::bytes(png_data.to_vec())
            .file_name(filename.to_string())
            .mime_str("image/png")
            .map_err(|e| ArcaferryError::ValidationError(e.to_string()))?;

        let form = multipart::Form::new().part("file", part);

        let mut request = self
            .client
            .post(&url)
            .header("X-Arcaferry-Version", ARCAFERRY_VERSION)
            .multipart(form);

        if let Some(ref token) = self.api_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let response = request.send().await.map_err(|e| {
            if e.is_connect() {
                ArcaferryError::ArcamageConnectionFailed(e.to_string())
            } else {
                ArcaferryError::NetworkError(e.to_string())
            }
        })?;

        self.handle_response(response).await
    }

    /// Test connection to Arcamage
    pub async fn test_connection(&self) -> Result<bool> {
        let url = format!("{}/api/health", self.base_url);

        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// Handle response and convert to ImportResponse
    async fn handle_response(&self, response: wreq::Response) -> Result<ImportResponse> {
        let status = response.status();
        let body: ImportResponse = response
            .json()
            .await
            .map_err(|e| ArcaferryError::InvalidJson(e.to_string()))?;

        if !body.success {
            // Check for specific error codes
            if let Some(ref code) = body.error_code {
                match code.as_str() {
                    "VERSION_MISMATCH" => {
                        return Err(ArcaferryError::VersionMismatch {
                            expected: "unknown".to_string(),
                            actual: ARCAFERRY_VERSION.to_string(),
                        });
                    }
                    "UNAUTHORIZED" => {
                        return Err(ArcaferryError::Unauthorized(
                            body.message.unwrap_or_default(),
                        ));
                    }
                    _ => {
                        return Err(ArcaferryError::ImportRejected(
                            body.message.unwrap_or_else(|| code.clone()),
                        ));
                    }
                }
            }

            if !status.is_success() {
                return Err(ArcaferryError::ImportRejected(
                    body.message.unwrap_or_else(|| format!("HTTP {}", status)),
                ));
            }
        }

        Ok(body)
    }

    /// Get the configured base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Update base URL
    pub fn set_base_url(&mut self, url: &str) {
        self.base_url = url.trim_end_matches('/').to_string();
    }

    /// Update API token
    pub fn set_api_token(&mut self, token: Option<&str>) {
        self.api_token = token.map(|s| s.to_string());
    }
}

impl Default for ArcamageClient {
    fn default() -> Self {
        Self::new(None, None).expect("Failed to create default Arcamage client")
    }
}
