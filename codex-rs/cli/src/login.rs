use codex_common::CliConfigOverrides;
use codex_core::CodexAuth;
use codex_core::auth::CLIENT_ID;
use codex_core::auth::OAuthProviderToken;
use codex_core::auth::get_auth_file;
use codex_core::auth::login_with_api_key;
use codex_core::auth::logout;
use codex_core::auth::write_auth_json;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::oauth_device_flow;
use codex_login::ServerOptions;
use codex_login::run_login_server;
use codex_protocol::mcp_protocol::AuthMode;
use std::path::PathBuf;

pub async fn login_with_chatgpt(codex_home: PathBuf) -> std::io::Result<()> {
    let opts = ServerOptions::new(codex_home, CLIENT_ID.to_string());
    let server = run_login_server(opts)?;

    eprintln!(
        "Starting local login server on http://localhost:{}.\nIf your browser did not open, navigate to this URL to authenticate:\n\n{}",
        server.actual_port, server.auth_url,
    );

    server.block_until_done().await
}

pub async fn run_login_with_chatgpt(cli_config_overrides: CliConfigOverrides) -> ! {
    let config = load_config_or_exit(cli_config_overrides);

    match login_with_chatgpt(config.codex_home).await {
        Ok(_) => {
            eprintln!("Successfully logged in");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Error logging in: {e}");
            std::process::exit(1);
        }
    }
}

/// Generic OAuth login for any provider with oauth_device_flow configured
pub async fn run_login_with_oauth_provider(
    cli_config_overrides: CliConfigOverrides,
    provider_id: String,
) -> ! {
    let config = load_config_or_exit(cli_config_overrides);

    // Load all providers (built-in + user-configured)
    let providers = &config.model_providers;
    let provider = match providers.get(&provider_id) {
        Some(p) => p,
        None => {
            eprintln!("Provider '{provider_id}' not found. Available providers with OAuth:");
            for (id, p) in providers.iter() {
                if p.oauth_device_flow.is_some() {
                    eprintln!("  - {id}");
                }
            }
            std::process::exit(1);
        }
    };

    let oauth_config = match &provider.oauth_device_flow {
        Some(cfg) => cfg,
        None => {
            eprintln!("Provider '{provider_id}' does not support OAuth device flow");
            std::process::exit(1);
        }
    };

    eprintln!("Logging in to {}...", provider.name);

    match oauth_device_flow::run_device_flow(oauth_config, &provider.name).await {
        Ok(token) => {
            // Handle optional token exchange
            let (final_token, expires_in) =
                if let Some(exchange_url) = &oauth_config.token_exchange_url {
                    match oauth_device_flow::exchange_token(
                        &token.access_token,
                        exchange_url,
                        oauth_config.token_exchange_headers.as_ref(),
                        oauth_config.token_exchange_env_headers.as_ref(),
                    )
                    .await
                    {
                        Ok((exchanged, exp)) => (exchanged, exp.or(token.expires_in)),
                        Err(e) => {
                            eprintln!("Token exchange failed: {e}");
                            std::process::exit(1);
                        }
                    }
                } else {
                    (token.access_token.clone(), token.expires_in)
                };

            // Save to auth.json
            let auth_file = get_auth_file(&config.codex_home);
            let mut auth_json = codex_core::auth::read_auth_json(&auth_file).unwrap_or_else(|_| {
                codex_core::auth::AuthDotJson {
                    openai_api_key: None,
                    oauth_tokens: std::collections::HashMap::new(),
                    githubcopilot_api_key: None,
                    github_token: None,
                    copilot_session_token: None,
                    copilot_token_expiration: None,
                    tokens: None,
                    last_refresh: None,
                }
            });

            let expires_at =
                expires_in.map(|secs| chrono::Utc::now() + chrono::Duration::seconds(secs as i64));
            auth_json.oauth_tokens.insert(
                provider_id.clone(),
                OAuthProviderToken {
                    access_token: token.access_token.clone(),
                    expires_at,
                    refresh_token: token.refresh_token,
                    session_token: if final_token != token.access_token {
                        Some(final_token)
                    } else {
                        None
                    },
                },
            );

            if let Err(e) = write_auth_json(&auth_file, &auth_json) {
                eprintln!("Error saving auth: {e}");
                std::process::exit(1);
            }

            eprintln!("Successfully logged in to {}", provider.name);
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("OAuth login failed: {e}");
            std::process::exit(1);
        }
    }
}

pub async fn run_login_with_api_key(
    cli_config_overrides: CliConfigOverrides,
    api_key: String,
) -> ! {
    let config = load_config_or_exit(cli_config_overrides);

    match login_with_api_key(&config.codex_home, &api_key) {
        Ok(_) => {
            eprintln!("Successfully logged in");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Error logging in: {e}");
            std::process::exit(1);
        }
    }
}

pub async fn run_login_status(cli_config_overrides: CliConfigOverrides) -> ! {
    let config = load_config_or_exit(cli_config_overrides);

    match CodexAuth::from_codex_home(&config.codex_home) {
        Ok(Some(auth)) => match auth.mode {
            AuthMode::ApiKey => match auth.get_token().await {
                Ok(api_key) => {
                    eprintln!("Logged in using an API key - {}", safe_format_key(&api_key));
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("Unexpected error retrieving API key: {e}");
                    std::process::exit(1);
                }
            },
            AuthMode::ChatGPT => {
                eprintln!("Logged in using ChatGPT");
                std::process::exit(0);
            }
        },
        Ok(None) => {
            eprintln!("Not logged in");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error checking login status: {e}");
            std::process::exit(1);
        }
    }
}

pub async fn run_logout(cli_config_overrides: CliConfigOverrides) -> ! {
    let config = load_config_or_exit(cli_config_overrides);

    match logout(&config.codex_home) {
        Ok(true) => {
            eprintln!("Successfully logged out");
            std::process::exit(0);
        }
        Ok(false) => {
            eprintln!("Not logged in");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Error logging out: {e}");
            std::process::exit(1);
        }
    }
}

fn load_config_or_exit(cli_config_overrides: CliConfigOverrides) -> Config {
    let cli_overrides = match cli_config_overrides.parse_overrides() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error parsing -c overrides: {e}");
            std::process::exit(1);
        }
    };

    let config_overrides = ConfigOverrides::default();
    match Config::load_with_cli_overrides(cli_overrides, config_overrides) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error loading configuration: {e}");
            std::process::exit(1);
        }
    }
}

fn safe_format_key(key: &str) -> String {
    if key.len() <= 13 {
        return "***".to_string();
    }
    let prefix = &key[..8];
    let suffix = &key[key.len() - 5..];
    format!("{prefix}***{suffix}")
}

#[cfg(test)]
mod tests {
    use super::safe_format_key;

    #[test]
    fn formats_long_key() {
        let key = "sk-proj-1234567890ABCDE";
        assert_eq!(safe_format_key(key), "sk-proj-***ABCDE");
    }

    #[test]
    fn short_key_returns_stars() {
        let key = "sk-proj-12345";
        assert_eq!(safe_format_key(key), "***");
    }
}
