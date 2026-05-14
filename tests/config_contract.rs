use assert_cmd::Command;
use confluence_cli::auth::{basic_auth_header, redacted_token};
use confluence_cli::command_context::CommandContext;
use confluence_cli::config::{config_path, load_config, save_config, Config};
use serde_json::json;
use std::fs;
use std::sync::{Mutex, MutexGuard};
use tempfile::tempdir;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

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
        default_space: Some("ENG".to_string()),
    };

    save_config(&path, &config).unwrap();
    let loaded = load_config(&path).unwrap();

    assert_eq!(loaded.site_url, "https://example.atlassian.net/wiki");
    assert_eq!(loaded.email, "user@example.com");
    assert_eq!(loaded.api_token, "token-value");
    assert_eq!(loaded.default_space.as_deref(), Some("ENG"));
    assert!(fs::metadata(path).unwrap().len() > 0);
}

#[test]
fn config_allows_missing_default_space() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(
        &path,
        r#"
site_url = "https://example.atlassian.net/wiki"
email = "user@example.com"
api_token = "token-value"
"#,
    )
    .unwrap();

    let loaded = load_config(&path).unwrap();

    assert_eq!(loaded.site_url, "https://example.atlassian.net/wiki");
    assert_eq!(loaded.email, "user@example.com");
    assert_eq!(loaded.api_token, "token-value");
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
        default_space: Some("ENG".to_string()),
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
        default_space: Some("ENG".to_string()),
    };

    let validated = config.validate().unwrap();

    assert_eq!(validated.site_url, "http://127.0.0.1:12345/wiki");
}

#[tokio::test]
async fn command_context_loads_client_from_config_env_var() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/spaces"))
        .and(query_param("limit", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"id": "ctx-123", "key": "CTX", "name": "Context Space"}],
            "_links": {}
        })))
        .mount(&server)
        .await;

    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let config = Config {
        site_url: server.uri(),
        email: "user@example.com".to_string(),
        api_token: "token-value".to_string(),
        default_space: Some("ENG".to_string()),
    };
    save_config(&path, &config).unwrap();

    let _guard = EnvVarGuard::set("CONFLUENCE_CLI_CONFIG", &path);
    let context = CommandContext::load().unwrap();
    let spaces = context.client().list_spaces().await.unwrap();

    assert_eq!(spaces[0].key, "CTX");
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

#[tokio::test]
async fn config_init_lists_spaces_and_selects_default_by_number() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/spaces"))
        .and(query_param("limit", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [
                {"id": "space-eng", "key": "ENG", "name": "Engineering"},
                {"id": "space-docs", "key": "DOCS", "name": "Documentation"}
            ],
            "_links": {}
        })))
        .mount(&server)
        .await;

    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");

    let output = Command::cargo_bin("confluence-cli")
        .unwrap()
        .arg("config")
        .arg("init")
        .env("CONFLUENCE_CLI_CONFIG", &path)
        .write_stdin(format!(
            "{}/\nuser@example.com\ntoken-value\n1\n",
            server.uri()
        ))
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(serde_json::from_str::<serde_json::Value>(&stdout).is_err());
    assert!(stdout.contains("Congratulations"));
    assert!(stdout.contains("setup is complete"));
    assert!(stdout.contains(path.to_str().unwrap()));
    assert!(!stdout.contains("token-value"));
    assert!(stderr.contains("Confluence site URL"));
    assert!(stderr.contains("Email"));
    assert!(stderr.contains("API token"));
    assert!(stderr.contains("Engineering"));
    assert!(stderr.contains("Documentation"));

    let loaded = load_config(&path).unwrap();
    assert_eq!(loaded.site_url, server.uri());
    assert_eq!(loaded.email, "user@example.com");
    assert_eq!(loaded.api_token, "token-value");
    assert_eq!(loaded.default_space.as_deref(), Some("ENG"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}

#[tokio::test]
async fn config_init_allows_no_spaces_without_default_space() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/spaces"))
        .and(query_param("limit", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [],
            "_links": {}
        })))
        .mount(&server)
        .await;

    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");

    let output = Command::cargo_bin("confluence-cli")
        .unwrap()
        .arg("config")
        .arg("init")
        .env("CONFLUENCE_CLI_CONFIG", &path)
        .write_stdin(format!(
            "{}/\nuser@example.com\ntoken-value\n",
            server.uri()
        ))
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    let written = fs::read_to_string(&path).unwrap();

    assert!(serde_json::from_str::<serde_json::Value>(&stdout).is_err());
    assert!(stdout.contains("Congratulations"));
    assert!(stdout.contains(path.to_str().unwrap()));
    assert!(stderr.contains("No accessible spaces"));
    assert!(!written.contains("default_space"));
}

#[tokio::test]
async fn config_init_exits_without_writing_config_when_api_verification_fails() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/spaces"))
        .and(query_param("limit", "25"))
        .respond_with(ResponseTemplate::new(401).set_body_string("bad credentials"))
        .mount(&server)
        .await;

    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");

    let output = Command::cargo_bin("confluence-cli")
        .unwrap()
        .arg("config")
        .arg("init")
        .env("CONFLUENCE_CLI_CONFIG", &path)
        .write_stdin(format!(
            "{}/\nuser@example.com\ntoken-value\n",
            server.uri()
        ))
        .output()
        .unwrap();

    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "auth_failed");
    assert!(value["error"]["message"]
        .as_str()
        .unwrap()
        .contains("config init"));
    assert!(!path.exists());
}

#[tokio::test]
async fn command_warns_when_loaded_config_has_no_default_space() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/spaces"))
        .and(query_param("limit", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"id": "space-eng", "key": "ENG", "name": "Engineering"}],
            "_links": {}
        })))
        .mount(&server)
        .await;

    let dir = tempdir().unwrap();
    let config = dir.path().join("config.toml");
    fs::write(
        &config,
        format!(
            r#"
site_url = "{}"
email = "user@example.com"
api_token = "token-value"
"#,
            server.uri()
        ),
    )
    .unwrap();

    let output = Command::cargo_bin("confluence-cli")
        .unwrap()
        .arg("space")
        .arg("list")
        .env("CONFLUENCE_CLI_CONFIG", &config)
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(value["ok"], true);
    assert!(stderr.contains("Warning"));
    assert!(stderr.contains("default_space"));
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
        default_space: Some("ENG".to_string()),
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
        default_space: Some("DOC".to_string()),
    };

    save_config(&path, &config).unwrap();

    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    let loaded = load_config(&path).unwrap();
    assert_eq!(mode, 0o600);
    assert_eq!(loaded, config.validate().unwrap());
}
