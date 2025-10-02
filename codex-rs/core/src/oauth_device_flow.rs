//! Generic OAuth 2.0 Device Authorization Grant (RFC 8628) implementation.

use crate::model_provider_info::OAuthDeviceFlowConfig;
use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct DeviceResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    #[serde(default)]
    interval: Option<u64>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
}

pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
}

pub async fn run_device_flow(
    config: &OAuthDeviceFlowConfig,
    provider_name: &str,
) -> Result<OAuthToken> {
    let client = Client::new();
    let mut device_req_body = serde_json::json!({"client_id": config.client_id});
    if let Some(scope) = &config.scope {
        device_req_body["scope"] = serde_json::Value::String(scope.clone());
    }
    let device_resp = client
        .post(&config.device_code_url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&device_req_body)
        .send()
        .await
        .context("failed to request device code")?;
    if !device_resp.status().is_success() {
        let status = device_resp.status();
        let text = device_resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "Device code request failed with status {status}: {text}"
        ));
    }
    let device: DeviceResponse = device_resp
        .json()
        .await
        .context("failed to parse device code response")?;
    let _ = open::that(&device.verification_uri);
    eprintln!(
        "\nTo authorize {provider_name}, visit:\n  {}\nand enter code: {}\n",
        device.verification_uri, device.user_code
    );
    let interval = std::time::Duration::from_secs(device.interval.unwrap_or(5));
    let mut elapsed = 0u64;
    loop {
        if elapsed > device.expires_in {
            return Err(anyhow!("Device code expired"));
        }
        tokio::time::sleep(interval).await;
        elapsed += interval.as_secs();
        let token_resp = client
            .post(&config.token_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&serde_json::json!({
                "client_id": config.client_id,
                "device_code": device.device_code,
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code"
            }))
            .send()
            .await;
        let parsed: TokenResponse = match token_resp {
            Ok(r) => match r.json().await {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("Failed to parse token response: {e}; continuing polling");
                    continue;
                }
            },
            Err(e) => {
                eprintln!("Failed to poll token endpoint: {e}; continuing polling");
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
                return Err(anyhow!("Device flow failed: {err}"));
            }
        }
        if let Some(access_token) = parsed.access_token {
            eprintln!("Authorization successful!");
            return Ok(OAuthToken {
                access_token,
                refresh_token: parsed.refresh_token,
                expires_in: parsed.expires_in,
            });
        }
    }
}

pub async fn exchange_token(
    oauth_token: &str,
    exchange_url: &str,
    headers: Option<&HashMap<String, String>>,
    env_headers: Option<&HashMap<String, String>>,
) -> Result<(String, Option<u64>)> {
    let client = Client::new();
    let mut req = client
        .get(exchange_url)
        .header("Authorization", format!("bearer {oauth_token}"))
        .header("Accept", "application/json");
    if let Some(hdrs) = headers {
        for (key, value) in hdrs {
            req = req.header(key, value);
        }
    }
    if let Some(env_hdrs) = env_headers {
        for (key, env_var) in env_hdrs {
            if let Ok(value) = std::env::var(env_var) {
                if !value.trim().is_empty() {
                    req = req.header(key, value);
                }
            }
        }
    }
    let resp = req.send().await.context("failed to exchange token")?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "Token exchange failed with status {status}: {text}"
        ));
    }
    #[derive(Deserialize)]
    struct ExchangeResponse {
        token: String,
        #[serde(default)]
        expires_at: Option<i64>,
        #[serde(default)]
        refresh_in: Option<u64>,
        #[serde(default)]
        expires_in: Option<u64>,
    }
    let data: ExchangeResponse = resp
        .json()
        .await
        .context("failed to parse exchange response")?;
    let expires_in = data.refresh_in.or(data.expires_in).or_else(|| {
        data.expires_at.and_then(|ts| {
            let now = chrono::Utc::now().timestamp();
            if ts > now {
                Some((ts - now) as u64)
            } else {
                None
            }
        })
    });
    Ok((data.token, expires_in))
}
