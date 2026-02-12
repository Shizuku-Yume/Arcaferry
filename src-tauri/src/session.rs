use std::collections::HashMap;
use std::sync::RwLock;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::cookies::CookieJar;
use crate::error::{ArcaferryError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub platform: String,
    pub cookies: Option<String>,
    pub bearer_token: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl Session {
    pub fn new(platform: &str) -> Self {
        Self {
            platform: platform.to_string(),
            cookies: None,
            bearer_token: None,
            created_at: Utc::now(),
            expires_at: None,
        }
    }

    pub fn with_cookies(mut self, cookies: &CookieJar) -> Self {
        self.cookies = Some(cookies.to_header_string());
        self
    }

    pub fn with_bearer_token(mut self, token: &str) -> Self {
        self.bearer_token = Some(token.to_string());
        self
    }

    pub fn with_expiry(mut self, duration: Duration) -> Self {
        self.expires_at = Some(Utc::now() + duration);
        self
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    pub fn get_cookie_jar(&self) -> Result<Option<CookieJar>> {
        match &self.cookies {
            Some(cookie_str) if !cookie_str.is_empty() => Ok(Some(CookieJar::parse(cookie_str)?)),
            _ => Ok(None),
        }
    }
}

#[derive(Default)]
pub struct SessionManager {
    sessions: RwLock<HashMap<String, Session>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, session: Session) -> Result<()> {
        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| ArcaferryError::ValidationError("Lock poisoned".to_string()))?;
        sessions.insert(session.platform.clone(), session);
        Ok(())
    }

    pub fn get(&self, platform: &str) -> Result<Option<Session>> {
        let sessions = self
            .sessions
            .read()
            .map_err(|_| ArcaferryError::ValidationError("Lock poisoned".to_string()))?;

        match sessions.get(platform) {
            Some(session) => {
                if session.is_expired() {
                    Ok(None)
                } else {
                    Ok(Some(session.clone()))
                }
            }
            None => Ok(None),
        }
    }

    pub fn remove(&self, platform: &str) -> Result<Option<Session>> {
        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| ArcaferryError::ValidationError("Lock poisoned".to_string()))?;
        Ok(sessions.remove(platform))
    }

    pub fn has_valid_session(&self, platform: &str) -> bool {
        self.get(platform).map(|s| s.is_some()).unwrap_or(false)
    }

    pub fn clear(&self) -> Result<()> {
        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| ArcaferryError::ValidationError("Lock poisoned".to_string()))?;
        sessions.clear();
        Ok(())
    }

    pub fn list_platforms(&self) -> Result<Vec<String>> {
        let sessions = self
            .sessions
            .read()
            .map_err(|_| ArcaferryError::ValidationError("Lock poisoned".to_string()))?;
        Ok(sessions.keys().cloned().collect())
    }
}

lazy_static::lazy_static! {
    pub static ref SESSIONS: SessionManager = SessionManager::new();
}
