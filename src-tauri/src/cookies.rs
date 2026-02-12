use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::{ArcaferryError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default, rename = "httpOnly")]
    pub http_only: bool,
    #[serde(default)]
    pub secure: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CookieJar {
    cookies: HashMap<String, Cookie>,
}

impl CookieJar {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse cookies from various formats:
    /// - JSON array format (EditThisCookie export)
    /// - Netscape format (cookies.txt)
    /// - Header string format (Cookie: key=value; key2=value2)
    pub fn parse(input: &str) -> Result<Self> {
        let trimmed = input.trim();

        if trimmed.is_empty() {
            return Ok(Self::new());
        }

        if trimmed.starts_with('[') {
            // JSON array format (EditThisCookie export)
            Self::parse_json(trimmed)
        } else if trimmed.contains('\t') || trimmed.starts_with('#') {
            // Netscape format
            Self::parse_netscape(trimmed)
        } else {
            // Header string format: "key=val; key2=val2"
            Self::parse_header_string(trimmed)
        }
    }

    /// Parse JSON array format (EditThisCookie export)
    fn parse_json(input: &str) -> Result<Self> {
        let cookies: Vec<serde_json::Value> =
            serde_json::from_str(input).map_err(|e| ArcaferryError::InvalidJson(e.to_string()))?;

        let mut jar = Self::new();

        for cookie_val in cookies {
            if let serde_json::Value::Object(obj) = cookie_val {
                let name = obj
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let value = obj
                    .get("value")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                if !name.is_empty() {
                    let cookie = Cookie {
                        name: name.clone(),
                        value,
                        domain: obj.get("domain").and_then(|v| v.as_str()).map(String::from),
                        path: obj.get("path").and_then(|v| v.as_str()).map(String::from),
                        http_only: obj
                            .get("httpOnly")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        secure: obj.get("secure").and_then(|v| v.as_bool()).unwrap_or(false),
                    };
                    jar.insert(cookie);
                }
            }
        }

        Ok(jar)
    }

    /// Parse Netscape format (cookies.txt)
    /// Format: domain\tflag\tpath\tsecure\texpiry\tname\tvalue
    fn parse_netscape(input: &str) -> Result<Self> {
        let mut jar = Self::new();

        for line in input.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 7 {
                let domain = parts[0].to_string();
                let path = parts[2].to_string();
                let secure = parts[3].to_lowercase() == "true";
                let name = parts[5].to_string();
                let value = parts[6].to_string();

                if !name.is_empty() {
                    let cookie = Cookie {
                        name: name.clone(),
                        value,
                        domain: Some(domain),
                        path: Some(path),
                        http_only: false, // Netscape format doesn't include httpOnly
                        secure,
                    };
                    jar.insert(cookie);
                }
            }
        }

        Ok(jar)
    }

    /// Parse header string format: "Cookie: key=val; key2=val2" or "key=val; key2=val2"
    fn parse_header_string(input: &str) -> Result<Self> {
        let mut jar = Self::new();

        // Strip "Cookie:" prefix if present
        let cookie_str = if input.to_lowercase().starts_with("cookie:") {
            input[7..].trim()
        } else {
            input
        };

        for pair in cookie_str.split(';') {
            let pair = pair.trim();
            if let Some(idx) = pair.find('=') {
                let name = pair[..idx].trim().to_string();
                let value = pair[idx + 1..].trim().to_string();

                if !name.is_empty() {
                    let cookie = Cookie {
                        name: name.clone(),
                        value,
                        domain: None,
                        path: None,
                        http_only: false,
                        secure: false,
                    };
                    jar.insert(cookie);
                }
            }
        }

        Ok(jar)
    }

    /// Convert to Cookie header string
    pub fn to_header_string(&self) -> String {
        self.cookies
            .values()
            .map(|c| format!("{}={}", c.name, c.value))
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// Get a specific cookie by name
    pub fn get(&self, name: &str) -> Option<&Cookie> {
        self.cookies.get(name)
    }

    /// Insert a cookie (replaces existing cookie with same name)
    pub fn insert(&mut self, cookie: Cookie) {
        self.cookies.insert(cookie.name.clone(), cookie);
    }

    /// Get all cookies as HashMap
    pub fn as_map(&self) -> &HashMap<String, Cookie> {
        &self.cookies
    }

    /// Check if jar is empty
    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty()
    }

    /// Get number of cookies
    pub fn len(&self) -> usize {
        self.cookies.len()
    }

    /// Convert to simple key-value HashMap (for HTTP client)
    pub fn to_simple_map(&self) -> HashMap<String, String> {
        self.cookies
            .values()
            .map(|c| (c.name.clone(), c.value.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_format() {
        let input = r#"[
            {"name": "session", "value": "abc123", "domain": ".example.com", "httpOnly": true},
            {"name": "token", "value": "xyz789", "secure": true}
        ]"#;

        let jar = CookieJar::parse(input).unwrap();
        assert_eq!(jar.len(), 2);

        let session = jar.get("session").unwrap();
        assert_eq!(session.value, "abc123");
        assert_eq!(session.domain.as_deref(), Some(".example.com"));
        assert!(session.http_only);

        let token = jar.get("token").unwrap();
        assert_eq!(token.value, "xyz789");
        assert!(token.secure);
    }

    #[test]
    fn test_parse_netscape_format() {
        let input = r#"# Netscape HTTP Cookie File
.example.com	TRUE	/	FALSE	1234567890	session	abc123
.example.com	TRUE	/api	TRUE	1234567890	token	xyz789"#;

        let jar = CookieJar::parse(input).unwrap();
        assert_eq!(jar.len(), 2);

        let session = jar.get("session").unwrap();
        assert_eq!(session.value, "abc123");
        assert_eq!(session.domain.as_deref(), Some(".example.com"));
        assert!(!session.secure);

        let token = jar.get("token").unwrap();
        assert_eq!(token.value, "xyz789");
        assert_eq!(token.path.as_deref(), Some("/api"));
        assert!(token.secure);
    }

    #[test]
    fn test_parse_header_string_format() {
        let input = "session=abc123; token=xyz789";
        let jar = CookieJar::parse(input).unwrap();
        assert_eq!(jar.len(), 2);
        assert_eq!(jar.get("session").unwrap().value, "abc123");
        assert_eq!(jar.get("token").unwrap().value, "xyz789");
    }

    #[test]
    fn test_parse_header_string_with_cookie_prefix() {
        let input = "Cookie: session=abc123; token=xyz789";
        let jar = CookieJar::parse(input).unwrap();
        assert_eq!(jar.len(), 2);
        assert_eq!(jar.get("session").unwrap().value, "abc123");
    }

    #[test]
    fn test_to_header_string() {
        let mut jar = CookieJar::new();
        jar.insert(Cookie {
            name: "a".to_string(),
            value: "1".to_string(),
            domain: None,
            path: None,
            http_only: false,
            secure: false,
        });

        let header = jar.to_header_string();
        assert!(header.contains("a=1"));
    }

    #[test]
    fn test_empty_input() {
        let jar = CookieJar::parse("").unwrap();
        assert!(jar.is_empty());

        let jar = CookieJar::parse("   ").unwrap();
        assert!(jar.is_empty());
    }
}
