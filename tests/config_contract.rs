use confluence_cli::auth::{basic_auth_header, redacted_token};
use confluence_cli::config::{config_path, load_config, save_config, Config};
use std::fs;
use tempfile::tempdir;

#[test]
fn env_var_overrides_default_config_path() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("custom.toml");

    std::env::set_var("CONFLUENCE_CLI_CONFIG", &path);
    let resolved = config_path().unwrap();
    std::env::remove_var("CONFLUENCE_CLI_CONFIG");

    assert_eq!(resolved, path);
}

#[test]
fn config_round_trip_trims_site_url_slash() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let config = Config {
        site_url: "https://example.atlassian.net/wiki/".to_string(),
        email: "user@example.com".to_string(),
        api_token: "token-value".to_string(),
        default_space: "ENG".to_string(),
    };

    save_config(&path, &config).unwrap();
    let loaded = load_config(&path).unwrap();

    assert_eq!(loaded.site_url, "https://example.atlassian.net/wiki");
    assert_eq!(loaded.email, "user@example.com");
    assert_eq!(loaded.api_token, "token-value");
    assert_eq!(loaded.default_space, "ENG");
    assert!(fs::metadata(path).unwrap().len() > 0);
}

#[test]
fn invalid_config_rejects_missing_fields() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(&path, "site_url = \"https://example.atlassian.net/wiki\"\n").unwrap();

    let error = load_config(&path).unwrap_err();

    assert_eq!(error.code.as_str(), "config_invalid");
}

#[test]
fn basic_auth_header_uses_email_and_token_without_redaction() {
    let value = basic_auth_header("user@example.com", "token-value").unwrap();

    assert!(value.to_str().unwrap().starts_with("Basic "));
    assert_ne!(value.to_str().unwrap(), "Basic [redacted]");
}

#[test]
fn redacted_token_never_returns_secret() {
    assert_eq!(redacted_token("abcdef"), "[redacted:6]");
}
