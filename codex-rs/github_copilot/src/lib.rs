use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use serde::Deserialize;
use std::env;
use std::io::{self, Write};

const GITHUB_DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GITHUB_COPILOT_CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";
const COPILOT_TOKEN_EXCHANGE_URL: &str = "https://api.github.com/copilot_internal/v2/token";

const USER_AGENT: &str = "GithubCopilot/1.155.0";
const EDITOR_VERSION: &str = "vscode/1.85.1";
const EDITOR_PLUGIN_VERSION: &str = "copilot/1.155.0";

#[derive(Deserialize)]
struct DeviceResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: Option<u64>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct CopilotTokenResponse {
    token: String,
    expires_at: Option<i64>,
    refresh_in: Option<u64>,
}

pub struct CopilotToken {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

pub async fn exchange_for_copilot_token(github_token: &str) -> Result<CopilotToken> {
    let client = Client::new();

    let resp = client
        .get(COPILOT_TOKEN_EXCHANGE_URL)
        .header("Authorization", format!("bearer {}", github_token))
        .header("Accept", "application/json")
        .header("User-Agent", USER_AGENT)
        .header("editor-version", EDITOR_VERSION)
        .header("editor-plugin-version", EDITOR_PLUGIN_VERSION)
        .send()
        .await
        .context("failed to exchange GitHub token for Copilot session token")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "Copilot token exchange failed with status {}: {}",
            status,
            text
        ));
    }

    let data: CopilotTokenResponse = resp
        .json()
        .await
        .context("failed to parse Copilot token response")?;

    let expires_at = if let Some(refresh_in) = data.refresh_in {
        Utc::now() + Duration::seconds(refresh_in as i64)
    } else if let Some(expires_at) = data.expires_at {
        DateTime::from_timestamp(expires_at, 0).unwrap_or_else(|| Utc::now() + Duration::hours(1))
    } else {
        Utc::now() + Duration::hours(1)
    };

    Ok(CopilotToken {
        token: data.token,
        expires_at,
    })
}

pub async fn get_device_flow_key() -> Result<String> {
    if let Ok(val) = env::var("GITHUBCOPILOT_API_KEY") {
        let trimmed = val.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    let device_url =
        env::var("COPILOT_DEVICE_CODE_URL").unwrap_or_else(|_| GITHUB_DEVICE_CODE_URL.to_string());
    let token_url = env::var("COPILOT_TOKEN_URL").unwrap_or_else(|_| GITHUB_TOKEN_URL.to_string());
    let client_id =
        env::var("COPILOT_CLIENT_ID").unwrap_or_else(|_| GITHUB_COPILOT_CLIENT_ID.to_string());

    let client = Client::new();

    let device_resp = client
        .post(&device_url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("User-Agent", USER_AGENT)
        .header("editor-version", EDITOR_VERSION)
        .header("editor-plugin-version", EDITOR_PLUGIN_VERSION)
        .json(&serde_json::json!({
            "client_id": client_id,
            "scope": "read:user"
        }))
        .send()
        .await;

    let resp = match device_resp {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            let status = r.status();
            eprintln!(
                "Device-code request to {} failed: {}. Falling back to prompt.",
                device_url, status
            );
            return fallback_interactive_prompt().await;
        }
        Err(e) => {
            eprintln!(
                "Device-code request to {} failed: {}. Falling back to prompt.",
                device_url, e
            );
            return fallback_interactive_prompt().await;
        }
    };

    let device: DeviceResponse = match resp.json().await {
        Ok(d) => d,
        Err(e) => {
            eprintln!(
                "Failed to parse device-code response: {}. Falling back to prompt.",
                e
            );
            return fallback_interactive_prompt().await;
        }
    };

    let _ = open::that(&device.verification_uri);

    eprintln!(
        "To authorize, visit {} and enter code {}",
        device.verification_uri, device.user_code
    );

    let interval = std::time::Duration::from_secs(device.interval.unwrap_or(5));
    let mut elapsed = 0u64;

    loop {
        if elapsed > device.expires_in {
            eprintln!("Device code expired; falling back to interactive prompt.");
            return fallback_interactive_prompt().await;
        }

        tokio::time::sleep(interval).await;
        elapsed += interval.as_secs();

        let token_resp = client
            .post(&token_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("User-Agent", USER_AGENT)
            .header("editor-version", EDITOR_VERSION)
            .header("editor-plugin-version", EDITOR_PLUGIN_VERSION)
            .json(&serde_json::json!({
                "client_id": client_id,
                "device_code": device.device_code,
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code"
            }))
            .send()
            .await;

        let parsed: TokenResponse = match token_resp {
            Ok(r) => match r.json().await {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("Failed to parse token response: {}; continuing polling", e);
                    continue;
                }
            },
            Err(e) => {
                eprintln!("Failed to poll token endpoint: {}; continuing polling", e);
                continue;
            }
        };

        if let Some(err) = parsed.error.as_ref() {
            if err == "authorization_pending" {
                continue;
            } else if err == "slow_down" {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            } else {
                eprintln!(
                    "Device flow failed: {}; falling back to interactive prompt",
                    err
                );
                return fallback_interactive_prompt().await;
            }
        }

        if let Some(github_token) = parsed.access_token {
            eprintln!("GitHub authorization successful.");
            return Ok(github_token);
        }
    }
}

async fn fallback_interactive_prompt() -> Result<String> {
    let _ = open::that("https://github.com/settings/tokens");

    let stdout = io::stdout();
    let mut handle = stdout.lock();
    writeln!(
        handle,
        "\nGitHub Copilot API key not found via env or device-flow.\n\
To continue, please create or obtain a Copilot/personal access token and paste it below.\n\
You can create a token here: https://github.com/settings/tokens\n"
    )
    .ok();

    print!("Paste Copilot API key and press Enter: ");
    handle.flush().ok();

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read API key from stdin")?;
    let key = input.trim().to_string();

    if key.is_empty() {
        Err(anyhow!("no GitHub Copilot API key provided"))
    } else {
        Ok(key)
    }
}

pub async fn get_github_copilot_device_key() -> Result<String> {
    get_device_flow_key().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn returns_env_key_when_set() {
        env::set_var("GITHUBCOPILOT_API_KEY", "test-key-123");
        let res = get_device_flow_key().await.unwrap();
        assert_eq!(res, "test-key-123");
        env::remove_var("GITHUBCOPILOT_API_KEY");
    }
}
