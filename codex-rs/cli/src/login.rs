use codex_common::CliConfigOverrides;
use codex_core::CodexAuth;
use codex_core::auth::CLIENT_ID;
use codex_core::auth::login_with_api_key;
use codex_core::auth::logout;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
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

/// Attempt to log in to GitHub Copilot using the device-code flow and persist the resulting
/// API key into the standard auth.json. This mirrors how other simple API-key providers are
/// handled: store the key and set the corresponding environment variable so provider configs
/// that reference `GITHUBCOPILOT_API_KEY` will work.
pub async fn run_login_with_github_copilot(cli_config_overrides: CliConfigOverrides) -> ! {
    let config = load_config_or_exit(cli_config_overrides);

    // The actual device-code flow helper is expected to live in a small crate/module
    // named `github_copilot` that exposes `get_device_flow_key() -> anyhow::Result<String>`.
    // If that helper is not present at build time, this will fail to compile; the helper
    // should implement the device-code polling logic (similar to the TS implementation).
    match github_copilot::get_device_flow_key().await {
        Ok(key) => {
            // Persist into auth.json so future runs can pick it up
            match codex_core::auth::login_with_githubcopilot_api_key(&config.codex_home, &key) {
                Ok(_) => {
                    // Make available to the running process as well
                    // Copilot key saved to auth.json. Do not set a process-wide environment
                    // variable here; users who need it for subprocesses can export the
                    // variable in their shell. This avoids issues with setting env vars
                    // in some execution contexts.
                    eprintln!("Successfully logged in (GitHub Copilot)");
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("Error saving GitHub Copilot API key: {e}");
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Error logging in to GitHub Copilot: {e}");
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
