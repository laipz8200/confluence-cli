use assert_cmd::Command;
use confluence_cli::auth::{basic_auth_header, redacted_token};
use confluence_cli::config::{config_path, load_config, save_config, Config};
use std::fs;
use std::sync::{Mutex, MutexGuard};
use tempfile::tempdir;

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvVarGuard {
    key: &'static str,
    original: Option<String>,
    _lock: MutexGuard<'static, ()>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &std::path::Path) -> Self {
        let lock = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let original = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self {
            key,
            original,
            _lock: lock,
        }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(value) = &self.original {
            std::env::set_var(self.key, value);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

#[test]
fn env_var_overrides_default_config_path() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("custom.toml");

    let _guard = EnvVarGuard::set("CONFLUENCE_CLI_CONFIG", &path);
    let resolved = config_path().unwrap();

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
fn config_rejects_non_loopback_http_site_url() {
    let config = Config {
        site_url: "http://example.atlassian.net/wiki".to_string(),
        email: "user@example.com".to_string(),
        api_token: "token-value".to_string(),
        default_space: "ENG".to_string(),
    };

    let error = config.validate().unwrap_err();

    assert_eq!(error.code.as_str(), "config_invalid");
    assert!(error.message.contains("https://"));
}

#[test]
fn config_allows_loopback_http_for_mock_servers() {
    let config = Config {
        site_url: "http://127.0.0.1:12345/wiki/".to_string(),
        email: "user@example.com".to_string(),
        api_token: "token-value".to_string(),
        default_space: "ENG".to_string(),
    };

    let validated = config.validate().unwrap();

    assert_eq!(validated.site_url, "http://127.0.0.1:12345/wiki");
}

#[test]
fn basic_auth_header_uses_email_and_token_without_redaction() {
    let value = basic_auth_header("user@example.com", "token-value").unwrap();

    assert!(value.to_str().unwrap().starts_with("Basic "));
    assert_ne!(value.to_str().unwrap(), "Basic [redacted]");
}

#[test]
fn redacted_token_never_returns_secret() {
    assert_eq!(redacted_token("abcdef"), "[redacted]");
}

#[test]
fn config_init_accepts_piped_stdin_and_keeps_stdout_json_only() {
    let _lock = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");

    let output = Command::cargo_bin("confluence-cli")
        .unwrap()
        .arg("config")
        .arg("init")
        .env("CONFLUENCE_CLI_CONFIG", &path)
        .write_stdin("https://example.atlassian.net/wiki/\nuser@example.com\ntoken-value\nENG\n")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(value["ok"], true);
    assert_eq!(value["command"], "config.init");
    assert!(!stdout.contains("token-value"));
    assert!(stderr.contains("Confluence site URL"));
    assert!(stderr.contains("Email"));
    assert!(stderr.contains("API token"));
    assert!(stderr.contains("Default space key"));

    let loaded = load_config(&path).unwrap();
    assert_eq!(loaded.site_url, "https://example.atlassian.net/wiki");
    assert_eq!(loaded.email, "user@example.com");
    assert_eq!(loaded.api_token, "token-value");
    assert_eq!(loaded.default_space, "ENG");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}

#[cfg(unix)]
#[test]
fn newly_created_config_file_has_owner_only_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let config = Config {
        site_url: "https://example.atlassian.net/wiki".to_string(),
        email: "user@example.com".to_string(),
        api_token: "token-value".to_string(),
        default_space: "ENG".to_string(),
    };

    save_config(&path, &config).unwrap();

    let mode = fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[cfg(unix)]
#[test]
fn rewriting_existing_config_file_tightens_permissions_before_writing() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(&path, "old token-bearing content").unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o644);
    fs::set_permissions(&path, permissions).unwrap();

    let config = Config {
        site_url: "https://example.atlassian.net/wiki/".to_string(),
        email: "new-user@example.com".to_string(),
        api_token: "new-token-value".to_string(),
        default_space: "DOC".to_string(),
    };

    save_config(&path, &config).unwrap();

    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    let loaded = load_config(&path).unwrap();
    assert_eq!(mode, 0o600);
    assert_eq!(loaded, config.validate().unwrap());
}
