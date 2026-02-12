use crate::adapters::quack::QuackAttribute;
use crate::error::{ArcaferryError, Result};

use std::path::PathBuf;
use std::process::{Command as StdCommand, Stdio};

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| {
            let v = v.trim();
            !v.is_empty() && v != "0" && v != "false" && v != "False"
        })
        .unwrap_or(false)
}

#[derive(Debug, Clone)]
pub enum BrowserCapability {
    Available,
    NotInstalled { reason: String },
    Error { reason: String },
}

#[derive(Debug, Clone)]
pub struct SidecarHiddenSettings {
    pub attrs: Vec<QuackAttribute>,
    pub stderr: String,
}

pub struct SidecarInvokeParams<'a> {
    pub cookies: Option<&'a str>,
    pub bearer_token: Option<&'a str>,
    pub gemini_api_key: Option<&'a str>,
    pub user_agent: Option<&'a str>,
    pub dream_url: Option<&'a str>,
}

fn sidecar_script_path() -> PathBuf {
    if let Ok(p) = std::env::var("ARCAFERRY_SIDECAR_SCRIPT_PATH") {
        let p = p.trim();
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    // This crate lives under `src-tauri/`. Sidecar scripts are at repo root `scripts/`.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("../scripts/extract_hidden.py")
}

fn run_python_check(args: &[&str]) -> std::io::Result<std::process::Output> {
    StdCommand::new("python3")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
}

/// Detect whether Python + Camoufox + Playwright(Firefox) are available.
///
/// This is an optional feature gate. When unavailable, the server should degrade gracefully.
pub fn detect_browser_capability() -> BrowserCapability {
    let script_path = sidecar_script_path();
    if !script_path.exists() {
        return BrowserCapability::NotInstalled {
            reason: format!("Sidecar script not found: {}", script_path.display()),
        };
    }

    let python_ok = run_python_check(&["--version"]);
    let python_out = match python_ok {
        Ok(o) => o,
        Err(e) => {
            return BrowserCapability::NotInstalled {
                reason: format!("python3 not found: {}", e),
            }
        }
    };
    if !python_out.status.success() {
        let stderr = String::from_utf8_lossy(&python_out.stderr);
        return BrowserCapability::Error {
            reason: format!("python3 failed: {}", stderr.trim()),
        };
    }

    // Check Camoufox module.
    let camoufox_out = match run_python_check(&["-c", "import camoufox.async_api"]) {
        Ok(o) => o,
        Err(e) => {
            return BrowserCapability::Error {
                reason: format!("Failed to execute python3 import check: {}", e),
            }
        }
    };
    if !camoufox_out.status.success() {
        return BrowserCapability::NotInstalled {
            reason: "Python module missing: camoufox".to_string(),
        };
    }

    // Check Playwright module.
    let playwright_out = match run_python_check(&["-c", "import playwright.async_api"]) {
        Ok(o) => o,
        Err(e) => {
            return BrowserCapability::Error {
                reason: format!("Failed to execute python3 import check: {}", e),
            }
        }
    };
    if !playwright_out.status.success() {
        return BrowserCapability::NotInstalled {
            reason: "Python module missing: playwright".to_string(),
        };
    }

    // Check Playwright Firefox installation.
    // We avoid launching the browser; we only validate executable path exists.
    let firefox_check = r#"
import os
import sys
from playwright.sync_api import sync_playwright

p = sync_playwright().start()
path = p.firefox.executable_path
p.stop()
sys.exit(0 if path and os.path.exists(path) else 1)
"#;
    let firefox_out = match run_python_check(&["-c", firefox_check]) {
        Ok(o) => o,
        Err(e) => {
            return BrowserCapability::Error {
                reason: format!("Failed to execute firefox check: {}", e),
            }
        }
    };
    if !firefox_out.status.success() {
        return BrowserCapability::NotInstalled {
            reason: "Playwright Firefox not installed (run: python -m playwright install firefox)".to_string(),
        };
    }

    BrowserCapability::Available
}

/// Call Python sidecar to extract hidden settings from share URL.
///
/// The script is expected to print a JSON array of QuackAttribute-like objects to stdout.
pub async fn extract_hidden_settings_via_sidecar(
    share_url: &str,
    hidden_labels: &[String],
    params: SidecarInvokeParams<'_>,
) -> Result<SidecarHiddenSettings> {
    use tokio::process::Command as TokioCommand;
    use tokio::{
        io::{AsyncBufReadExt, AsyncReadExt, BufReader},
        task::JoinHandle,
    };

    let timeout_secs: u64 = std::env::var("ARCAFERRY_SIDECAR_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(300);

    let script_path = sidecar_script_path();
    if !script_path.exists() {
        return Err(ArcaferryError::BrowserError(format!(
            "Sidecar script not found: {}",
            script_path.display()
        )));
    }

    let labels_json = serde_json::to_string(hidden_labels)
        .map_err(|e| ArcaferryError::InvalidJson(format!("Failed to encode labels: {}", e)))?;

    let sidecar_headed = env_flag("ARCAFERRY_SIDECAR_HEADED");
    let sidecar_trace = env_flag("ARCAFERRY_SIDECAR_TRACE");

    let mut cmd = TokioCommand::new("python3");
    cmd.arg(script_path)
        .arg("--url")
        .arg(share_url)
        .arg("--labels")
        .arg(labels_json);

    // Keep Python logs unbuffered so trace mode streams in real time.
    cmd.env("PYTHONUNBUFFERED", "1");

    if sidecar_headed {
        // Alias of --no-headless.
        cmd.arg("--headed");
    }
    if sidecar_trace {
        // Enable detailed step logs to stderr.
        cmd.arg("--trace");
    }

    if let Some(c) = params.cookies {
        if !c.trim().is_empty() {
            cmd.arg("--cookies").arg(c);
        }
    }
    if let Some(t) = params.bearer_token {
        if !t.trim().is_empty() {
            cmd.arg("--token").arg(t);
        }
    }

    if let Some(k) = params.gemini_api_key {
        if !k.trim().is_empty() {
            cmd.arg("--gemini-api-key").arg(k);
        }
    }

    if let Some(ua) = params.user_agent {
        if !ua.trim().is_empty() {
            cmd.arg("--user-agent").arg(ua);
        }
    }

    if let Some(dream) = params.dream_url {
        if !dream.trim().is_empty() {
            cmd.arg("--dream-url").arg(dream);
        }
    }

    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Ensure the child process is terminated if we time out and drop the future.
    cmd.kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|e| {
        ArcaferryError::BrowserError(format!("Failed to spawn python sidecar: {}", e))
    })?;

    let output = if !sidecar_trace {
        match tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            child.wait_with_output(),
        )
        .await
        {
            Ok(r) => {
                r.map_err(|e| ArcaferryError::BrowserError(format!("Sidecar failed: {}", e)))?
            }
            Err(_) => {
                // kill_on_drop(true) handles termination.
                return Err(ArcaferryError::BrowserError(format!(
                    "Sidecar timeout ({}s)",
                    timeout_secs
                )));
            }
        }
    } else {
        // Trace mode: stream stderr line-by-line to server logs while collecting output.
        let mut stdout = child.stdout.take().ok_or_else(|| {
            ArcaferryError::BrowserError("Sidecar stdout not captured".to_string())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            ArcaferryError::BrowserError("Sidecar stderr not captured".to_string())
        })?;

        let stderr_task: JoinHandle<std::io::Result<String>> = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            let mut collected = String::new();
            while let Ok(Some(line)) = lines.next_line().await {
                let line = line.trim_end().to_string();
                if !line.is_empty() {
                    tracing::info!(target = "arcaferry_lib::sidecar", line = %line, "sidecar");
                    collected.push_str(&line);
                    collected.push('\n');
                }
            }
            Ok(collected)
        });

        let stdout_task: JoinHandle<std::io::Result<Vec<u8>>> = tokio::spawn(async move {
            let mut buf = Vec::new();
            stdout.read_to_end(&mut buf).await?;
            Ok(buf)
        });

        let status = match tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            child.wait(),
        )
        .await
        {
            Ok(r) => r.map_err(|e| ArcaferryError::BrowserError(format!("Sidecar failed: {}", e)))?,
            Err(_) => {
                let _ = child.kill().await;
                stderr_task.abort();
                stdout_task.abort();
                return Err(ArcaferryError::BrowserError(format!(
                    "Sidecar timeout ({}s)",
                    timeout_secs
                )));
            }
        };

        let stdout_buf = stdout_task
            .await
            .map_err(|e| ArcaferryError::BrowserError(format!("Sidecar stdout join failed: {}", e)))?
            .map_err(|e| ArcaferryError::BrowserError(format!("Sidecar stdout read failed: {}", e)))?;

        let stderr_str = stderr_task
            .await
            .map_err(|e| ArcaferryError::BrowserError(format!("Sidecar stderr join failed: {}", e)))?
            .map_err(|e| ArcaferryError::BrowserError(format!("Sidecar stderr read failed: {}", e)))?;

        std::process::Output {
            status,
            stdout: stdout_buf,
            stderr: stderr_str.into_bytes(),
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ArcaferryError::BrowserError(format!(
            "Sidecar exited with {}: {}",
            output.status,
            stderr.trim()
        )));
    }

    let stderr_str = String::from_utf8_lossy(&output.stderr).trim().to_string();

    let parsed = serde_json::from_slice::<Vec<QuackAttribute>>(&output.stdout).map_err(|e| {
        let stdout = String::from_utf8_lossy(&output.stdout);
        ArcaferryError::BrowserError(format!(
            "Failed to parse sidecar JSON output: {} (stdout: {})",
            e,
            stdout.trim()
        ))
    })?;

    Ok(SidecarHiddenSettings {
        attrs: parsed,
        stderr: stderr_str,
    })
}

#[cfg(test)]
mod tests {
    // In tests we intentionally serialize env-var mutations across await points.
    #![allow(clippy::await_holding_lock)]

    use super::*;

    lazy_static::lazy_static! {
        static ref ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    }

    #[tokio::test]
    async fn sidecar_timeout_is_reported() {
        let _guard = ENV_LOCK.lock().unwrap();
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let script = manifest_dir.join("../scripts/sidecar_sleep.py");
        assert!(script.exists());

        std::env::set_var("ARCAFERRY_SIDECAR_SCRIPT_PATH", script.to_string_lossy().to_string());
        std::env::set_var("ARCAFERRY_SIDECAR_TIMEOUT_SECS", "1");

        let err = extract_hidden_settings_via_sidecar(
            "https://example.invalid/share",
            &["Body".to_string()],
            SidecarInvokeParams {
                cookies: None,
                bearer_token: None,
                gemini_api_key: None,
                user_agent: None,
                dream_url: None,
            },
        )
        .await
        .unwrap_err();

        match err {
            ArcaferryError::BrowserError(s) => {
                assert!(
                    s.to_lowercase().contains("timeout"),
                    "expected timeout error, got: {}",
                    s
                );
            }
            other => panic!("expected BrowserError, got: {}", other),
        }

        std::env::remove_var("ARCAFERRY_SIDECAR_SCRIPT_PATH");
        std::env::remove_var("ARCAFERRY_SIDECAR_TIMEOUT_SECS");
    }

    #[tokio::test]
    async fn sidecar_missing_script_is_reported() {
        let _guard = ENV_LOCK.lock().unwrap();
        // Use a random path to avoid collisions with real files.
        let missing = format!(
            "/tmp/arcaferry-missing-sidecar-{}.py",
            uuid::Uuid::new_v4()
        );
        // Best-effort cleanup if somehow exists.
        let _ = std::fs::remove_file(&missing);
        std::env::set_var("ARCAFERRY_SIDECAR_SCRIPT_PATH", &missing);
        std::env::remove_var("ARCAFERRY_SIDECAR_TIMEOUT_SECS");
        let err = extract_hidden_settings_via_sidecar(
            "https://example.invalid/share",
            &["Body".to_string()],
            SidecarInvokeParams {
                cookies: None,
                bearer_token: None,
                gemini_api_key: None,
                user_agent: None,
                dream_url: None,
            },
        )
        .await
        .unwrap_err();

        let msg = format!("{}", err);
        assert!(msg.to_lowercase().contains("script"));

        std::env::remove_var("ARCAFERRY_SIDECAR_SCRIPT_PATH");
    }
}
