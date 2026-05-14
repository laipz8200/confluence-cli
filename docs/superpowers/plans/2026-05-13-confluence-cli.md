# Confluence CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first release of `confluence-cli`, a Rust CLI that lets Agents search, read, create, and update Confluence Cloud pages through stable JSON and safe dry-run writes.

**Architecture:** The CLI is a small Rust binary backed by focused modules for CLI parsing, config, auth, HTTP client behavior, content conversion, dry-run payloads, command orchestration, and JSON output. Commands call Confluence Cloud REST API v2 for spaces and pages, and REST API v1 for CQL search. A generic Skills package documents how Agents should invoke the binary and gate real writes behind explicit user approval.

**Tech Stack:** Rust 2021, clap, tokio, reqwest, serde, serde_json, toml, thiserror, rpassword, base64, pulldown-cmark, html-escape, tempfile, assert_cmd, predicates, wiremock, pretty_assertions.

---

## File Structure

Create these files:

- `.gitignore`: Rust build artifacts and local secret files.
- `Cargo.toml`: crate metadata, runtime dependencies, and test dependencies.
- `README.md`: source-build installation, configuration, command examples, safety model, and Skills installation.
- `examples/config.toml`: non-secret sample config.
- `src/main.rs`: async entry point and process exit.
- `src/lib.rs`: public module wiring and top-level `run`.
- `src/cli.rs`: clap command definitions and command dispatch.
- `src/error.rs`: stable error codes, retry flags, and app error type.
- `src/output.rs`: JSON success/error envelope construction and stdout printing.
- `src/config.rs`: config path resolution, TOML load/save, validation, and `config init`.
- `src/auth.rs`: Basic Auth header construction with token redaction helpers.
- `src/client.rs`: Confluence client, endpoint construction, HTTP methods, status mapping, pagination, and API DTOs.
- `src/content.rs`: Markdown to storage HTML conversion and heading extraction.
- `src/dry_run.rs`: dry-run summary and sanitized payload preview for writes.
- `src/commands/mod.rs`: command module exports.
- `src/commands/config.rs`: `config init` orchestration.
- `src/commands/space.rs`: `space list`.
- `src/commands/search.rs`: simplified query and raw CQL search.
- `src/commands/page.rs`: `page get`, `page create`, and `page update`.
- `skills/confluence-cli/SKILL.md`: installable Skills instructions.
- `skills/confluence-cli/skill.json`: Skills package manifest.
- `tests/cli_smoke.rs`: binary smoke tests.
- `tests/config_contract.rs`: config and auth behavior.
- `tests/content_contract.rs`: Markdown conversion and dry-run summary behavior.
- `tests/http_contract.rs`: mocked HTTP client behavior.
- `tests/read_commands.rs`: mocked read-command CLI tests.
- `tests/write_commands.rs`: mocked write-command CLI tests.

Modify these files:

- `docs/superpowers/specs/2026-05-13-confluence-cli-design.md`: only if implementation discovers a spec contradiction. Record the reason in the commit message.

---

### Task 1: Rust Crate Baseline

**Files:**
- Create: `.gitignore`
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`
- Create: `src/cli.rs`
- Create: `tests/cli_smoke.rs`

- [ ] **Step 1: Write the smoke test**

Create `tests/cli_smoke.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn version_flag_prints_package_version() {
    let mut cmd = Command::cargo_bin("confluence-cli").unwrap();

    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_mentions_agent_first_commands() {
    let mut cmd = Command::cargo_bin("confluence-cli").unwrap();

    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("config"))
        .stdout(predicate::str::contains("space"))
        .stdout(predicate::str::contains("search"))
        .stdout(predicate::str::contains("page"));
}
```

- [ ] **Step 2: Run the smoke test to verify it fails**

Run:

```bash
cargo test --test cli_smoke
```

Expected: FAIL because `Cargo.toml` and the binary do not exist yet.

- [ ] **Step 3: Create crate metadata and baseline files**

Create `.gitignore`:

```gitignore
/target/
/.confluence-cli.toml
*.log
```

Create `Cargo.toml`:

```toml
[package]
name = "confluence-cli"
version = "0.1.0"
edition = "2021"
license = "Apache-2.0"
description = "Agent-friendly CLI for Confluence Cloud"
repository = "https://github.com/laipz8200/confluence-cli"

[dependencies]
base64 = "0.22"
clap = { version = "4", features = ["derive"] }
html-escape = "0.2"
pulldown-cmark = "0.12"
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
rpassword = "7"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
toml = "0.8"
url = "2"

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
pretty_assertions = "1"
tempfile = "3"
wiremock = "0.6"
```

Create `src/main.rs`:

```rust
#[tokio::main]
async fn main() {
    std::process::exit(confluence_cli::run().await);
}
```

Create `src/lib.rs`:

```rust
pub mod cli;

pub async fn run() -> i32 {
    cli::run().await
}
```

Create `src/cli.rs`:

```rust
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "confluence-cli")]
#[command(version)]
#[command(about = "Agent-friendly CLI for Confluence Cloud")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    Space {
        #[command(subcommand)]
        command: SpaceCommand,
    },
    Search {
        #[arg(long, conflicts_with = "cql")]
        query: Option<String>,
        #[arg(long, conflicts_with = "query")]
        cql: Option<String>,
    },
    Page {
        #[command(subcommand)]
        command: PageCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    Init,
}

#[derive(Debug, Subcommand)]
pub enum SpaceCommand {
    List,
}

#[derive(Debug, Subcommand)]
pub enum PageCommand {
    Get {
        #[arg(long)]
        page_id: String,
    },
    Create {
        #[arg(long)]
        space_key: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        body_file: std::path::PathBuf,
        #[arg(long)]
        parent_id: Option<String>,
        #[arg(long)]
        execute: bool,
    },
    Update {
        #[arg(long)]
        page_id: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        body_file: std::path::PathBuf,
        #[arg(long)]
        execute: bool,
    },
}

pub async fn run() -> i32 {
    let _cli = Cli::parse();
    0
}
```

- [ ] **Step 4: Run the smoke test to verify it passes**

Run:

```bash
cargo test --test cli_smoke
```

Expected: PASS for both smoke tests.

- [ ] **Step 5: Commit**

```bash
git add .gitignore Cargo.toml src/main.rs src/lib.rs src/cli.rs tests/cli_smoke.rs
git commit -m "chore: scaffold Rust CLI crate"
```

---

### Task 2: JSON Output and Error Contract

**Files:**
- Create: `src/error.rs`
- Create: `src/output.rs`
- Modify: `src/lib.rs`
- Test: `tests/output_contract.rs`

- [ ] **Step 1: Write the output contract tests**

Create `tests/output_contract.rs`:

```rust
use confluence_cli::error::{AppError, ErrorCode};
use confluence_cli::output::{error_json, success_json};
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn success_envelope_has_stable_shape() {
    let value = success_json("page.update", true, json!({"page_id": "123"}));

    assert_eq!(
        value,
        json!({
            "ok": true,
            "command": "page.update",
            "dry_run": true,
            "data": {"page_id": "123"}
        })
    );
}

#[test]
fn error_envelope_has_stable_shape() {
    let error = AppError::new(
        ErrorCode::ConfluenceVersionConflict,
        "Page was updated by someone else. Fetch the latest version and retry.",
    )
    .with_retryable(true)
    .with_details(json!({"status": 409}));

    let value = error_json("page.update", &error);

    assert_eq!(
        value,
        json!({
            "ok": false,
            "command": "page.update",
            "error": {
                "code": "confluence_version_conflict",
                "message": "Page was updated by someone else. Fetch the latest version and retry.",
                "retryable": true,
                "details": {"status": 409}
            }
        })
    );
}

#[test]
fn token_like_details_are_redacted() {
    let error = AppError::new(ErrorCode::AuthFailed, "Authentication failed.")
        .with_details(json!({
            "Authorization": "Basic abc123",
            "api_token": "secret",
            "nested": {"token": "secret"}
        }));

    let value = error_json("space.list", &error);
    let text = serde_json::to_string(&value).unwrap();

    assert!(!text.contains("abc123"));
    assert!(!text.contains("secret"));
    assert!(text.contains("[redacted]"));
}
```

- [ ] **Step 2: Run the output tests to verify they fail**

Run:

```bash
cargo test --test output_contract
```

Expected: FAIL because `src/error.rs` and `src/output.rs` are not defined.

- [ ] **Step 3: Implement error codes and output helpers**

Modify `src/lib.rs`:

```rust
pub mod cli;
pub mod error;
pub mod output;

pub async fn run() -> i32 {
    cli::run().await
}
```

Create `src/error.rs`:

```rust
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    ConfigNotFound,
    ConfigInvalid,
    AuthFailed,
    PermissionDenied,
    NotFound,
    RateLimited,
    ConfluenceValidationFailed,
    ConfluenceVersionConflict,
    NetworkError,
    MarkdownConversionFailed,
    UnsupportedMarkdown,
    InternalError,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::ConfigNotFound => "config_not_found",
            ErrorCode::ConfigInvalid => "config_invalid",
            ErrorCode::AuthFailed => "auth_failed",
            ErrorCode::PermissionDenied => "permission_denied",
            ErrorCode::NotFound => "not_found",
            ErrorCode::RateLimited => "rate_limited",
            ErrorCode::ConfluenceValidationFailed => "confluence_validation_failed",
            ErrorCode::ConfluenceVersionConflict => "confluence_version_conflict",
            ErrorCode::NetworkError => "network_error",
            ErrorCode::MarkdownConversionFailed => "markdown_conversion_failed",
            ErrorCode::UnsupportedMarkdown => "unsupported_markdown",
            ErrorCode::InternalError => "internal_error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub retryable: bool,
    pub details: Value,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: false,
            details: Value::Object(Map::new()),
        }
    }

    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = redact_value(details);
        self
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for AppError {}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
    pub retryable: bool,
    pub details: Value,
}

impl From<&AppError> for ErrorBody {
    fn from(value: &AppError) -> Self {
        Self {
            code: value.code.as_str(),
            message: value.message.clone(),
            retryable: value.retryable,
            details: value.details.clone(),
        }
    }
}

pub fn redact_value(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let lower = key.to_ascii_lowercase();
                    if lower.contains("authorization")
                        || lower.contains("api_token")
                        || lower == "token"
                    {
                        (key, Value::String("[redacted]".to_string()))
                    } else {
                        (key, redact_value(value))
                    }
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(redact_value).collect()),
        other => other,
    }
}
```

Create `src/output.rs`:

```rust
use crate::error::{AppError, ErrorBody};
use serde::Serialize;
use serde_json::{json, Value};

pub fn success_json<T: Serialize>(command: &'static str, dry_run: bool, data: T) -> Value {
    json!({
        "ok": true,
        "command": command,
        "dry_run": dry_run,
        "data": data
    })
}

pub fn error_json(command: &'static str, error: &AppError) -> Value {
    let body = ErrorBody::from(error);
    json!({
        "ok": false,
        "command": command,
        "error": body
    })
}

pub fn print_json(value: &Value) -> Result<(), AppError> {
    let text = serde_json::to_string_pretty(value).map_err(|source| {
        AppError::new(
            crate::error::ErrorCode::InternalError,
            format!("Failed to serialize JSON output: {source}"),
        )
    })?;
    println!("{text}");
    Ok(())
}
```

- [ ] **Step 4: Run the output tests to verify they pass**

Run:

```bash
cargo test --test output_contract
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/error.rs src/output.rs tests/output_contract.rs
git commit -m "feat: add JSON output contract"
```

---

### Task 3: Configuration and Auth

**Files:**
- Create: `src/config.rs`
- Create: `src/auth.rs`
- Modify: `src/lib.rs`
- Modify: `src/cli.rs`
- Create: `src/commands/mod.rs`
- Create: `src/commands/config.rs`
- Test: `tests/config_contract.rs`

- [ ] **Step 1: Write config and auth tests**

Create `tests/config_contract.rs`:

```rust
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
```

- [ ] **Step 2: Run config tests to verify they fail**

Run:

```bash
cargo test --test config_contract
```

Expected: FAIL because config and auth modules do not exist.

- [ ] **Step 3: Implement config and auth modules**

Modify `src/lib.rs`:

```rust
pub mod auth;
pub mod cli;
pub mod commands;
pub mod config;
pub mod error;
pub mod output;

pub async fn run() -> i32 {
    cli::run().await
}
```

Create `src/config.rs`:

```rust
use crate::error::{AppError, ErrorCode};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    pub site_url: String,
    pub email: String,
    pub api_token: String,
    pub default_space: String,
}

impl Config {
    pub fn validate(mut self) -> Result<Self, AppError> {
        self.site_url = self.site_url.trim_end_matches('/').to_string();
        if self.site_url.is_empty()
            || self.email.is_empty()
            || self.api_token.is_empty()
            || self.default_space.is_empty()
        {
            return Err(AppError::new(
                ErrorCode::ConfigInvalid,
                "Config must include site_url, email, api_token, and default_space.",
            ));
        }
        if !self.site_url.starts_with("https://") && !self.site_url.starts_with("http://") {
            return Err(AppError::new(
                ErrorCode::ConfigInvalid,
                "Config site_url must start with http:// or https://.",
            ));
        }
        Ok(self)
    }
}

pub fn config_path() -> Result<PathBuf, AppError> {
    if let Ok(path) = std::env::var("CONFLUENCE_CLI_CONFIG") {
        return Ok(PathBuf::from(path));
    }

    let home = std::env::var_os("HOME").ok_or_else(|| {
        AppError::new(
            ErrorCode::ConfigInvalid,
            "HOME is not set and CONFLUENCE_CLI_CONFIG was not provided.",
        )
    })?;

    Ok(PathBuf::from(home)
        .join(".config")
        .join("confluence-cli")
        .join("config.toml"))
}

pub fn load_default_config() -> Result<Config, AppError> {
    let path = config_path()?;
    load_config(&path)
}

pub fn load_config(path: &Path) -> Result<Config, AppError> {
    let text = std::fs::read_to_string(path).map_err(|source| {
        let code = if source.kind() == std::io::ErrorKind::NotFound {
            ErrorCode::ConfigNotFound
        } else {
            ErrorCode::ConfigInvalid
        };
        AppError::new(code, format!("Failed to read config at {}.", path.display()))
    })?;
    let config: Config = toml::from_str(&text).map_err(|source| {
        AppError::new(
            ErrorCode::ConfigInvalid,
            format!("Failed to parse config TOML: {source}"),
        )
    })?;
    config.validate()
}

pub fn save_config(path: &Path, config: &Config) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| {
            AppError::new(
                ErrorCode::ConfigInvalid,
                format!("Failed to create config directory {}: {source}", parent.display()),
            )
        })?;
    }

    let normalized = config.clone().validate()?;
    let text = toml::to_string_pretty(&normalized).map_err(|source| {
        AppError::new(
            ErrorCode::ConfigInvalid,
            format!("Failed to serialize config TOML: {source}"),
        )
    })?;
    std::fs::write(path, text).map_err(|source| {
        AppError::new(
            ErrorCode::ConfigInvalid,
            format!("Failed to write config at {}: {source}", path.display()),
        )
    })?;
    set_owner_only_permissions(path)?;
    Ok(())
}

#[cfg(unix)]
fn set_owner_only_permissions(path: &Path) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)
        .map_err(|source| {
            AppError::new(
                ErrorCode::ConfigInvalid,
                format!("Failed to read config permissions: {source}"),
            )
        })?
        .permissions();
    permissions.set_mode(0o600);
    std::fs::set_permissions(path, permissions).map_err(|source| {
        AppError::new(
            ErrorCode::ConfigInvalid,
            format!("Failed to set config permissions: {source}"),
        )
    })
}

#[cfg(not(unix))]
fn set_owner_only_permissions(_path: &Path) -> Result<(), AppError> {
    Ok(())
}
```

Create `src/auth.rs`:

```rust
use crate::error::{AppError, ErrorCode};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use reqwest::header::{HeaderValue, AUTHORIZATION};

pub fn basic_auth_header(email: &str, api_token: &str) -> Result<HeaderValue, AppError> {
    let encoded = STANDARD.encode(format!("{email}:{api_token}"));
    HeaderValue::from_str(&format!("Basic {encoded}")).map_err(|source| {
        AppError::new(
            ErrorCode::ConfigInvalid,
            format!("Failed to build authorization header: {source}"),
        )
    })
}

pub fn auth_header_name() -> reqwest::header::HeaderName {
    AUTHORIZATION
}

pub fn redacted_token(api_token: &str) -> String {
    format!("[redacted:{}]", api_token.len())
}
```

Create `src/commands/mod.rs`:

```rust
pub mod config;
```

Create `src/commands/config.rs`:

```rust
use crate::config::{config_path, save_config, Config};
use crate::error::AppError;
use serde_json::json;
use std::io::{self, Write};

pub fn init() -> Result<serde_json::Value, AppError> {
    let site_url = prompt("Confluence site URL")?;
    let email = prompt("Email")?;
    let api_token = rpassword::prompt_password("API token: ").map_err(|source| {
        crate::error::AppError::new(
            crate::error::ErrorCode::ConfigInvalid,
            format!("Failed to read API token: {source}"),
        )
    })?;
    let default_space = prompt("Default space key")?;

    let config = Config {
        site_url,
        email,
        api_token,
        default_space,
    };
    let path = config_path()?;
    save_config(&path, &config)?;

    Ok(json!({
        "path": path,
        "site_url": config.site_url.trim_end_matches('/'),
        "email": config.email,
        "default_space": config.default_space
    }))
}

fn prompt(label: &str) -> Result<String, AppError> {
    print!("{label}: ");
    io::stdout().flush().map_err(|source| {
        AppError::new(
            crate::error::ErrorCode::ConfigInvalid,
            format!("Failed to flush prompt: {source}"),
        )
    })?;
    let mut value = String::new();
    io::stdin().read_line(&mut value).map_err(|source| {
        AppError::new(
            crate::error::ErrorCode::ConfigInvalid,
            format!("Failed to read {label}: {source}"),
        )
    })?;
    Ok(value.trim().to_string())
}
```

- [ ] **Step 4: Wire `config init` into CLI dispatch**

Replace `src/cli.rs` with:

```rust
use crate::error::{AppError, ErrorCode};
use crate::output::{error_json, print_json, success_json};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "confluence-cli")]
#[command(version)]
#[command(about = "Agent-friendly CLI for Confluence Cloud")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    Space {
        #[command(subcommand)]
        command: SpaceCommand,
    },
    Search {
        #[arg(long, conflicts_with = "cql")]
        query: Option<String>,
        #[arg(long, conflicts_with = "query")]
        cql: Option<String>,
    },
    Page {
        #[command(subcommand)]
        command: PageCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    Init,
}

#[derive(Debug, Subcommand)]
pub enum SpaceCommand {
    List,
}

#[derive(Debug, Subcommand)]
pub enum PageCommand {
    Get {
        #[arg(long)]
        page_id: String,
    },
    Create {
        #[arg(long)]
        space_key: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        body_file: std::path::PathBuf,
        #[arg(long)]
        parent_id: Option<String>,
        #[arg(long)]
        execute: bool,
    },
    Update {
        #[arg(long)]
        page_id: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        body_file: std::path::PathBuf,
        #[arg(long)]
        execute: bool,
    },
}

pub async fn run() -> i32 {
    let cli = Cli::parse();
    let result = dispatch(cli).await;

    match result {
        Ok((command, dry_run, data)) => match print_json(&success_json(command, dry_run, data)) {
            Ok(()) => 0,
            Err(error) => {
                let _ = print_json(&error_json(command, &error));
                1
            }
        },
        Err((command, error)) => {
            let fallback = if command.is_empty() {
                "unknown"
            } else {
                command
            };
            let _ = print_json(&error_json(fallback, &error));
            1
        }
    }
}

async fn dispatch(cli: Cli) -> Result<(&'static str, bool, serde_json::Value), (&'static str, AppError)> {
    match cli.command {
        Commands::Config {
            command: ConfigCommand::Init,
        } => crate::commands::config::init()
            .map(|data| ("config.init", false, data))
            .map_err(|error| ("config.init", error)),
        Commands::Space { .. } => Err((
            "space.list",
            AppError::new(ErrorCode::InternalError, "space list is unavailable in this incremental build."),
        )),
        Commands::Search { .. } => Err((
            "search",
            AppError::new(ErrorCode::InternalError, "search is unavailable in this incremental build."),
        )),
        Commands::Page { command } => {
            let name = match command {
                PageCommand::Get { .. } => "page.get",
                PageCommand::Create { .. } => "page.create",
                PageCommand::Update { .. } => "page.update",
            };
            Err((name, AppError::new(ErrorCode::InternalError, format!("{name} is unavailable in this incremental build."))))
        }
    }
}
```

- [ ] **Step 5: Run config tests to verify they pass**

Run:

```bash
cargo test --test config_contract
cargo test --test cli_smoke
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/lib.rs src/config.rs src/auth.rs src/cli.rs src/commands/mod.rs src/commands/config.rs tests/config_contract.rs
git commit -m "feat: add config and auth foundations"
```

---

### Task 4: Markdown Conversion and Dry-Run Summaries

**Files:**
- Create: `src/content.rs`
- Create: `src/dry_run.rs`
- Modify: `src/lib.rs`
- Test: `tests/content_contract.rs`

- [ ] **Step 1: Write content and dry-run tests**

Create `tests/content_contract.rs`:

```rust
use confluence_cli::content::markdown_to_storage;
use confluence_cli::dry_run::{create_dry_run, WriteTarget};
use pretty_assertions::assert_eq;

#[test]
fn markdown_conversion_supports_common_subset() {
    let converted = markdown_to_storage("# Title\n\nA **bold** [link](https://example.com).\n\n- one\n- two\n").unwrap();

    assert!(converted.storage_html.contains("<h1>Title</h1>"));
    assert!(converted.storage_html.contains("<strong>bold</strong>"));
    assert!(converted.storage_html.contains("<a href=\"https://example.com\">link</a>"));
    assert!(converted.storage_html.contains("<ul>"));
    assert_eq!(converted.headings, vec!["Title"]);
}

#[test]
fn unsupported_image_returns_stable_error_code() {
    let error = markdown_to_storage("![alt](image.png)").unwrap_err();

    assert_eq!(error.code.as_str(), "unsupported_markdown");
}

#[test]
fn dry_run_summary_excludes_full_body() {
    let converted = markdown_to_storage("# Title\n\nBody").unwrap();
    let summary = create_dry_run(
        "POST",
        "/api/v2/pages",
        WriteTarget::Create {
            space_key: "ENG".to_string(),
            space_id: "987".to_string(),
            parent_id: Some("123".to_string()),
        },
        "Title",
        &converted,
    );
    let text = serde_json::to_string(&summary).unwrap();

    assert!(text.contains("\"method\":\"POST\""));
    assert!(text.contains("\"space_key\":\"ENG\""));
    assert!(text.contains("\"storage_html_bytes\""));
    assert!(!text.contains("<h1>Title</h1>"));
}
```

- [ ] **Step 2: Run content tests to verify they fail**

Run:

```bash
cargo test --test content_contract
```

Expected: FAIL because `content` and `dry_run` modules do not exist.

- [ ] **Step 3: Implement Markdown conversion**

Modify `src/lib.rs`:

```rust
pub mod auth;
pub mod cli;
pub mod commands;
pub mod config;
pub mod content;
pub mod dry_run;
pub mod error;
pub mod output;

pub async fn run() -> i32 {
    cli::run().await
}
```

Create `src/content.rs`:

```rust
use crate::error::{AppError, ErrorCode};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertedContent {
    pub storage_html: String,
    pub markdown_bytes: usize,
    pub storage_html_bytes: usize,
    pub headings: Vec<String>,
}

pub fn markdown_to_storage(markdown: &str) -> Result<ConvertedContent, AppError> {
    reject_unsupported(markdown)?;

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(markdown, options);
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, parser);

    Ok(ConvertedContent {
        markdown_bytes: markdown.len(),
        storage_html_bytes: html.len(),
        headings: collect_headings(markdown),
        storage_html: html,
    })
}

fn reject_unsupported(markdown: &str) -> Result<(), AppError> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    let parser = Parser::new_ext(markdown, options);
    for event in parser {
        match event {
            Event::Start(Tag::Image { .. }) => {
                return Err(AppError::new(
                    ErrorCode::UnsupportedMarkdown,
                    "Images and attachments are not supported by the first release.",
                ));
            }
            Event::Html(_) | Event::InlineHtml(_) => {
                return Err(AppError::new(
                    ErrorCode::UnsupportedMarkdown,
                    "Raw HTML is not supported by the first release.",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn collect_headings(markdown: &str) -> Vec<String> {
    let parser = Parser::new_ext(markdown, Options::ENABLE_TABLES);
    let mut headings = Vec::new();
    let mut current = String::new();
    let mut in_heading = false;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                in_heading = true;
                current.clear();
            }
            Event::Text(text) | Event::Code(text) if in_heading => {
                current.push_str(&text);
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
                let heading = current.trim();
                if !heading.is_empty() {
                    headings.push(heading.to_string());
                }
                if headings.len() == 5 {
                    break;
                }
            }
            _ => {}
        }
    }

    headings
}
```

- [ ] **Step 4: Implement dry-run summaries**

Create `src/dry_run.rs`:

```rust
use crate::content::ConvertedContent;
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WriteTarget {
    Create {
        space_key: String,
        space_id: String,
        parent_id: Option<String>,
    },
    Update {
        page_id: String,
        current_version: u64,
        next_version: u64,
    },
}

pub fn create_dry_run(
    method: &'static str,
    endpoint: impl Into<String>,
    target: WriteTarget,
    title: &str,
    content: &ConvertedContent,
) -> Value {
    json!({
        "method": method,
        "endpoint": endpoint.into(),
        "target": target,
        "title": title,
        "body": {
            "format": "storage",
            "summary": {
                "markdown_bytes": content.markdown_bytes,
                "storage_html_bytes": content.storage_html_bytes,
                "headings": content.headings,
            }
        },
        "payload_preview": {
            "status": "current",
            "title": title,
            "body": {
                "representation": "storage",
                "value": "[omitted]"
            }
        }
    })
}
```

- [ ] **Step 5: Run content tests to verify they pass**

Run:

```bash
cargo test --test content_contract
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/lib.rs src/content.rs src/dry_run.rs tests/content_contract.rs
git commit -m "feat: add markdown conversion and dry-run summaries"
```

---

### Task 5: Confluence HTTP Client

**Files:**
- Create: `src/client.rs`
- Modify: `src/lib.rs`
- Test: `tests/http_contract.rs`

- [ ] **Step 1: Write HTTP client tests**

Create `tests/http_contract.rs`:

```rust
use confluence_cli::client::{ConfluenceClient, CreatePageRequest, UpdatePageRequest};
use confluence_cli::config::Config;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::matchers::{basic_auth, body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn config(base_url: &str) -> Config {
    Config {
        site_url: base_url.to_string(),
        email: "user@example.com".to_string(),
        api_token: "token-value".to_string(),
        default_space: "ENG".to_string(),
    }
}

#[tokio::test]
async fn list_spaces_calls_v2_spaces_with_auth() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/spaces"))
        .and(query_param("limit", "25"))
        .and(basic_auth("user@example.com", "token-value"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"id": "987", "key": "ENG", "name": "Engineering"}],
            "_links": {}
        })))
        .mount(&server)
        .await;

    let client = ConfluenceClient::new(config(&server.uri())).unwrap();
    let spaces = client.list_spaces().await.unwrap();

    assert_eq!(spaces[0].id, "987");
    assert_eq!(spaces[0].key, "ENG");
}

#[tokio::test]
async fn resolve_space_key_returns_matching_space_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/spaces"))
        .and(query_param("keys", "ENG"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"id": "987", "key": "ENG", "name": "Engineering"}],
            "_links": {}
        })))
        .mount(&server)
        .await;

    let client = ConfluenceClient::new(config(&server.uri())).unwrap();
    let space_id = client.resolve_space_id("ENG").await.unwrap();

    assert_eq!(space_id, "987");
}

#[tokio::test]
async fn search_uses_v1_cql_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/search"))
        .and(query_param("cql", "text ~ \"deploy\""))
        .and(query_param("limit", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"content": {"id": "123", "title": "Deploy Guide"}}]
        })))
        .mount(&server)
        .await;

    let client = ConfluenceClient::new(config(&server.uri())).unwrap();
    let results = client.search("text ~ \"deploy\"").await.unwrap();

    assert_eq!(results["results"][0]["content"]["id"], "123");
}

#[tokio::test]
async fn create_page_sends_v2_payload() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/pages"))
        .and(body_json(json!({
            "spaceId": "987",
            "status": "current",
            "title": "New Page",
            "parentId": "123",
            "body": {"representation": "storage", "value": "<h1>New Page</h1>\n"},
            "subtype": "live"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "456"})))
        .mount(&server)
        .await;

    let client = ConfluenceClient::new(config(&server.uri())).unwrap();
    let response = client
        .create_page(CreatePageRequest {
            space_id: "987".to_string(),
            title: "New Page".to_string(),
            parent_id: Some("123".to_string()),
            storage_html: "<h1>New Page</h1>\n".to_string(),
        })
        .await
        .unwrap();

    assert_eq!(response["id"], "456");
}

#[tokio::test]
async fn update_page_sends_next_version_payload() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/pages/456"))
        .and(body_json(json!({
            "id": "456",
            "status": "current",
            "title": "Updated Page",
            "body": {"representation": "storage", "value": "<h1>Updated Page</h1>\n"},
            "version": {"number": 8, "message": "Updated by confluence-cli"}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "456"})))
        .mount(&server)
        .await;

    let client = ConfluenceClient::new(config(&server.uri())).unwrap();
    let response = client
        .update_page(UpdatePageRequest {
            page_id: "456".to_string(),
            title: "Updated Page".to_string(),
            next_version: 8,
            storage_html: "<h1>Updated Page</h1>\n".to_string(),
        })
        .await
        .unwrap();

    assert_eq!(response["id"], "456");
}

#[tokio::test]
async fn status_mapping_returns_stable_error_code() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/spaces"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"message": "Unauthorized"})))
        .mount(&server)
        .await;

    let client = ConfluenceClient::new(config(&server.uri())).unwrap();
    let error = client.list_spaces().await.unwrap_err();

    assert_eq!(error.code.as_str(), "auth_failed");
}
```

- [ ] **Step 2: Run HTTP tests to verify they fail**

Run:

```bash
cargo test --test http_contract
```

Expected: FAIL because `src/client.rs` does not exist.

- [ ] **Step 3: Implement the HTTP client and DTOs**

Modify `src/lib.rs`:

```rust
pub mod auth;
pub mod cli;
pub mod client;
pub mod commands;
pub mod config;
pub mod content;
pub mod dry_run;
pub mod error;
pub mod output;

pub async fn run() -> i32 {
    cli::run().await
}
```

Create `src/client.rs`:

```rust
use crate::auth::{auth_header_name, basic_auth_header};
use crate::config::Config;
use crate::error::{AppError, ErrorCode};
use reqwest::header::{HeaderMap, ACCEPT, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use url::Url;

#[derive(Clone)]
pub struct ConfluenceClient {
    http: reqwest::Client,
    base_url: Url,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Space {
    pub id: String,
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct MultiResult<T> {
    results: Vec<T>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Page {
    pub id: String,
    pub status: String,
    pub title: String,
    #[serde(rename = "spaceId")]
    pub space_id: Option<String>,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
    pub version: Option<PageVersion>,
    pub body: Option<Value>,
    #[serde(rename = "_links")]
    pub links: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PageVersion {
    pub number: u64,
}

#[derive(Debug, Clone)]
pub struct CreatePageRequest {
    pub space_id: String,
    pub title: String,
    pub parent_id: Option<String>,
    pub storage_html: String,
}

#[derive(Debug, Clone)]
pub struct UpdatePageRequest {
    pub page_id: String,
    pub title: String,
    pub next_version: u64,
    pub storage_html: String,
}

impl ConfluenceClient {
    pub fn new(config: Config) -> Result<Self, AppError> {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, "application/json".parse().unwrap());
        headers.insert(auth_header_name(), basic_auth_header(&config.email, &config.api_token)?);

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|source| {
                AppError::new(
                    ErrorCode::InternalError,
                    format!("Failed to build HTTP client: {source}"),
                )
            })?;

        let base_url = Url::parse(config.site_url.trim_end_matches('/')).map_err(|source| {
            AppError::new(
                ErrorCode::ConfigInvalid,
                format!("Invalid Confluence site_url: {source}"),
            )
        })?;

        Ok(Self { http, base_url })
    }

    pub async fn list_spaces(&self) -> Result<Vec<Space>, AppError> {
        let url = self.url("/api/v2/spaces")?;
        let response = self
            .http
            .get(url)
            .query(&[("limit", "25")])
            .send()
            .await
            .map_err(network_error)?;
        let body: MultiResult<Space> = self.parse(response, "space.list").await?;
        Ok(body.results)
    }

    pub async fn resolve_space_id(&self, key: &str) -> Result<String, AppError> {
        let url = self.url("/api/v2/spaces")?;
        let response = self
            .http
            .get(url)
            .query(&[("keys", key), ("limit", "1")])
            .send()
            .await
            .map_err(network_error)?;
        let body: MultiResult<Space> = self.parse(response, "space.resolve").await?;
        body.results
            .into_iter()
            .find(|space| space.key == key)
            .map(|space| space.id)
            .ok_or_else(|| AppError::new(ErrorCode::NotFound, format!("Space key {key} was not found.")))
    }

    pub async fn search(&self, cql: &str) -> Result<Value, AppError> {
        let url = self.url("/rest/api/search")?;
        let response = self
            .http
            .get(url)
            .query(&[("cql", cql), ("limit", "25")])
            .send()
            .await
            .map_err(network_error)?;
        self.parse(response, "search").await
    }

    pub async fn get_page(&self, page_id: &str) -> Result<Page, AppError> {
        let url = self.url(&format!("/api/v2/pages/{page_id}"))?;
        let response = self
            .http
            .get(url)
            .query(&[("body-format", "storage")])
            .send()
            .await
            .map_err(network_error)?;
        self.parse(response, "page.get").await
    }

    pub async fn create_page(&self, request: CreatePageRequest) -> Result<Value, AppError> {
        let mut payload = json!({
            "spaceId": request.space_id,
            "status": "current",
            "title": request.title,
            "body": {"representation": "storage", "value": request.storage_html},
            "subtype": "live"
        });
        if let Some(parent_id) = request.parent_id {
            payload["parentId"] = json!(parent_id);
        }

        let response = self
            .http
            .post(self.url("/api/v2/pages")?)
            .header(CONTENT_TYPE, "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(network_error)?;
        self.parse(response, "page.create").await
    }

    pub async fn update_page(&self, request: UpdatePageRequest) -> Result<Value, AppError> {
        let payload = json!({
            "id": request.page_id,
            "status": "current",
            "title": request.title,
            "body": {"representation": "storage", "value": request.storage_html},
            "version": {"number": request.next_version, "message": "Updated by confluence-cli"}
        });

        let response = self
            .http
            .put(self.url(&format!("/api/v2/pages/{}", payload["id"].as_str().unwrap()))?)
            .header(CONTENT_TYPE, "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(network_error)?;
        self.parse(response, "page.update").await
    }

    fn url(&self, path: &str) -> Result<Url, AppError> {
        let base = self.base_url.as_str().trim_end_matches('/');
        Url::parse(&format!("{base}{path}")).map_err(|source| {
            AppError::new(
                ErrorCode::ConfigInvalid,
                format!("Failed to build Confluence URL for {path}: {source}"),
            )
        })
    }

    async fn parse<T: for<'de> Deserialize<'de>>(
        &self,
        response: reqwest::Response,
        command: &'static str,
    ) -> Result<T, AppError> {
        let status = response.status();
        let text = response.text().await.map_err(network_error)?;
        if !status.is_success() {
            return Err(error_from_status(status.as_u16(), command, text));
        }
        serde_json::from_str(&text).map_err(|source| {
            AppError::new(
                ErrorCode::ConfluenceValidationFailed,
                format!("Failed to parse Confluence response: {source}"),
            )
        })
    }
}

fn network_error(source: reqwest::Error) -> AppError {
    AppError::new(ErrorCode::NetworkError, format!("Network request failed: {source}"))
        .with_retryable(true)
}

pub fn error_from_status(status: u16, command: &'static str, body: String) -> AppError {
    let code = match status {
        401 => ErrorCode::AuthFailed,
        403 => ErrorCode::PermissionDenied,
        404 => ErrorCode::NotFound,
        409 if command == "page.update" => ErrorCode::ConfluenceVersionConflict,
        429 => ErrorCode::RateLimited,
        500..=599 => ErrorCode::NetworkError,
        _ => ErrorCode::ConfluenceValidationFailed,
    };
    let retryable = matches!(code, ErrorCode::RateLimited | ErrorCode::NetworkError | ErrorCode::ConfluenceVersionConflict);
    AppError::new(code, format!("Confluence returned HTTP {status}."))
        .with_retryable(retryable)
        .with_details(json!({"status": status, "body": truncate(&body, 1200)}))
}

fn truncate(value: &str, max: usize) -> String {
    if value.len() <= max {
        value.to_string()
    } else {
        format!("{}...[truncated]", &value[..max])
    }
}
```

- [ ] **Step 4: Run HTTP tests to verify they pass**

Run:

```bash
cargo test --test http_contract
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/client.rs tests/http_contract.rs
git commit -m "feat: add Confluence HTTP client"
```

---

### Task 6: Read Commands

**Files:**
- Create: `src/commands/space.rs`
- Create: `src/commands/search.rs`
- Create: `src/commands/page.rs`
- Modify: `src/commands/mod.rs`
- Modify: `src/cli.rs`
- Test: `tests/read_commands.rs`

- [ ] **Step 1: Write read-command integration tests**

Create `tests/read_commands.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use std::fs;
use tempfile::tempdir;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn write_config(dir: &tempfile::TempDir, site_url: &str) -> std::path::PathBuf {
    let path = dir.path().join("config.toml");
    fs::write(
        &path,
        format!(
            r#"
site_url = "{site_url}"
email = "user@example.com"
api_token = "token-value"
default_space = "ENG"
"#
        ),
    )
    .unwrap();
    path
}

#[tokio::test]
async fn space_list_prints_json() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/spaces"))
        .and(query_param("limit", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"id": "987", "key": "ENG", "name": "Engineering"}],
            "_links": {}
        })))
        .mount(&server)
        .await;
    let dir = tempdir().unwrap();
    let config = write_config(&dir, &server.uri());

    let mut cmd = Command::cargo_bin("confluence-cli").unwrap();
    cmd.env("CONFLUENCE_CLI_CONFIG", config)
        .args(["space", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ok\": true"))
        .stdout(predicate::str::contains("\"command\": \"space.list\""))
        .stdout(predicate::str::contains("\"key\": \"ENG\""));
}

#[tokio::test]
async fn search_query_builds_cql() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/search"))
        .and(query_param("cql", "text ~ \"deploy\""))
        .and(query_param("limit", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"content": {"id": "123", "title": "Deploy Guide"}}]
        })))
        .mount(&server)
        .await;
    let dir = tempdir().unwrap();
    let config = write_config(&dir, &server.uri());

    let mut cmd = Command::cargo_bin("confluence-cli").unwrap();
    cmd.env("CONFLUENCE_CLI_CONFIG", config)
        .args(["search", "--query", "deploy"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"command\": \"search\""))
        .stdout(predicate::str::contains("Deploy Guide"));
}

#[tokio::test]
async fn page_get_requests_storage_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/pages/123"))
        .and(query_param("body-format", "storage"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "123",
            "status": "current",
            "title": "Deploy Guide",
            "spaceId": "987",
            "version": {"number": 7},
            "body": {"storage": {"value": "<p>Body</p>"}},
            "_links": {"webui": "/spaces/ENG/pages/123"}
        })))
        .mount(&server)
        .await;
    let dir = tempdir().unwrap();
    let config = write_config(&dir, &server.uri());

    let mut cmd = Command::cargo_bin("confluence-cli").unwrap();
    cmd.env("CONFLUENCE_CLI_CONFIG", config)
        .args(["page", "get", "--page-id", "123"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"command\": \"page.get\""))
        .stdout(predicate::str::contains("\"title\": \"Deploy Guide\""));
}
```

- [ ] **Step 2: Run read-command tests to verify they fail**

Run:

```bash
cargo test --test read_commands
```

Expected: FAIL because command modules are not wired yet.

- [ ] **Step 3: Implement read command modules**

Replace `src/commands/mod.rs`:

```rust
pub mod config;
pub mod page;
pub mod search;
pub mod space;
```

Create `src/commands/space.rs`:

```rust
use crate::client::ConfluenceClient;
use crate::config::load_default_config;
use crate::error::AppError;
use serde_json::json;

pub async fn list() -> Result<serde_json::Value, AppError> {
    let config = load_default_config()?;
    let client = ConfluenceClient::new(config)?;
    let spaces = client.list_spaces().await?;
    Ok(json!({ "spaces": spaces }))
}
```

Create `src/commands/search.rs`:

```rust
use crate::client::ConfluenceClient;
use crate::config::load_default_config;
use crate::error::{AppError, ErrorCode};
use serde_json::json;

pub async fn run(query: Option<String>, cql: Option<String>) -> Result<serde_json::Value, AppError> {
    let cql = match (query, cql) {
        (Some(query), None) => simple_query_to_cql(&query),
        (None, Some(cql)) => cql,
        _ => {
            return Err(AppError::new(
                ErrorCode::ConfluenceValidationFailed,
                "Pass exactly one of --query or --cql.",
            ));
        }
    };

    let config = load_default_config()?;
    let client = ConfluenceClient::new(config)?;
    let result = client.search(&cql).await?;
    Ok(json!({ "cql": cql, "result": result }))
}

fn simple_query_to_cql(query: &str) -> String {
    let escaped = query.replace('\\', "\\\\").replace('"', "\\\"");
    format!("text ~ \"{escaped}\"")
}
```

Create `src/commands/page.rs`:

```rust
use crate::client::ConfluenceClient;
use crate::config::load_default_config;
use crate::error::AppError;

pub async fn get(page_id: &str) -> Result<serde_json::Value, AppError> {
    let config = load_default_config()?;
    let client = ConfluenceClient::new(config)?;
    let page = client.get_page(page_id).await?;
    serde_json::to_value(page).map_err(|source| {
        crate::error::AppError::new(
            crate::error::ErrorCode::InternalError,
            format!("Failed to serialize page response: {source}"),
        )
    })
}
```

- [ ] **Step 4: Wire read commands into CLI dispatch**

Edit the `dispatch` match in `src/cli.rs`:

```rust
async fn dispatch(cli: Cli) -> Result<(&'static str, bool, serde_json::Value), (&'static str, AppError)> {
    match cli.command {
        Commands::Config {
            command: ConfigCommand::Init,
        } => crate::commands::config::init()
            .map(|data| ("config.init", false, data))
            .map_err(|error| ("config.init", error)),
        Commands::Space {
            command: SpaceCommand::List,
        } => crate::commands::space::list()
            .await
            .map(|data| ("space.list", false, data))
            .map_err(|error| ("space.list", error)),
        Commands::Search { query, cql } => crate::commands::search::run(query, cql)
            .await
            .map(|data| ("search", false, data))
            .map_err(|error| ("search", error)),
        Commands::Page { command } => match command {
            PageCommand::Get { page_id } => crate::commands::page::get(&page_id)
                .await
                .map(|data| ("page.get", false, data))
                .map_err(|error| ("page.get", error)),
            PageCommand::Create { .. } => Err((
                "page.create",
                AppError::new(ErrorCode::InternalError, "page.create is unavailable in this incremental build."),
            )),
            PageCommand::Update { .. } => Err((
                "page.update",
                AppError::new(ErrorCode::InternalError, "page.update is unavailable in this incremental build."),
            )),
        },
    }
}
```

- [ ] **Step 5: Run read-command tests to verify they pass**

Run:

```bash
cargo test --test read_commands
cargo test --test http_contract
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/commands/mod.rs src/commands/space.rs src/commands/search.rs src/commands/page.rs src/cli.rs tests/read_commands.rs
git commit -m "feat: add read commands"
```

---

### Task 7: Page Create With Dry-Run and Execute

**Files:**
- Modify: `src/commands/page.rs`
- Modify: `src/cli.rs`
- Test: `tests/write_commands.rs`

- [ ] **Step 1: Write page-create integration tests**

Create `tests/write_commands.rs` with these initial tests:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use std::fs;
use tempfile::tempdir;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn write_config(dir: &tempfile::TempDir, site_url: &str) -> std::path::PathBuf {
    let path = dir.path().join("config.toml");
    fs::write(
        &path,
        format!(
            r#"
site_url = "{site_url}"
email = "user@example.com"
api_token = "token-value"
default_space = "ENG"
"#
        ),
    )
    .unwrap();
    path
}

#[tokio::test]
async fn page_create_without_execute_is_dry_run_and_does_not_post() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/spaces"))
        .and(query_param("keys", "ENG"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"id": "987", "key": "ENG", "name": "Engineering"}],
            "_links": {}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempdir().unwrap();
    let config = write_config(&dir, &server.uri());
    let body = dir.path().join("page.md");
    fs::write(&body, "# New Page\n\nBody").unwrap();

    let mut cmd = Command::cargo_bin("confluence-cli").unwrap();
    cmd.env("CONFLUENCE_CLI_CONFIG", config)
        .args([
            "page",
            "create",
            "--space-key",
            "ENG",
            "--title",
            "New Page",
            "--body-file",
            body.to_str().unwrap(),
            "--parent-id",
            "123",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"command\": \"page.create\""))
        .stdout(predicate::str::contains("\"dry_run\": true"))
        .stdout(predicate::str::contains("\"endpoint\": \"/api/v2/pages\""));
}

#[tokio::test]
async fn page_create_execute_posts_payload() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/spaces"))
        .and(query_param("keys", "ENG"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"id": "987", "key": "ENG", "name": "Engineering"}],
            "_links": {}
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v2/pages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "456",
            "title": "New Page"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempdir().unwrap();
    let config = write_config(&dir, &server.uri());
    let body = dir.path().join("page.md");
    fs::write(&body, "# New Page\n\nBody").unwrap();

    let mut cmd = Command::cargo_bin("confluence-cli").unwrap();
    cmd.env("CONFLUENCE_CLI_CONFIG", config)
        .args([
            "page",
            "create",
            "--space-key",
            "ENG",
            "--title",
            "New Page",
            "--body-file",
            body.to_str().unwrap(),
            "--execute",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"dry_run\": false"))
        .stdout(predicate::str::contains("\"id\": \"456\""));
}
```

- [ ] **Step 2: Run write-command tests to verify create tests fail**

Run:

```bash
cargo test --test write_commands
```

Expected: FAIL because page create still returns an internal error.

- [ ] **Step 3: Implement page create**

Replace `src/commands/page.rs`:

```rust
use crate::client::{ConfluenceClient, CreatePageRequest, UpdatePageRequest};
use crate::config::load_default_config;
use crate::content::markdown_to_storage;
use crate::dry_run::{create_dry_run, WriteTarget};
use crate::error::{AppError, ErrorCode};
use std::path::Path;

pub async fn get(page_id: &str) -> Result<serde_json::Value, AppError> {
    let config = load_default_config()?;
    let client = ConfluenceClient::new(config)?;
    let page = client.get_page(page_id).await?;
    serde_json::to_value(page).map_err(|source| {
        crate::error::AppError::new(
            crate::error::ErrorCode::InternalError,
            format!("Failed to serialize page response: {source}"),
        )
    })
}

pub async fn create(
    space_key: &str,
    title: &str,
    body_file: &Path,
    parent_id: Option<String>,
    execute: bool,
) -> Result<(bool, serde_json::Value), AppError> {
    let config = load_default_config()?;
    let client = ConfluenceClient::new(config)?;
    let markdown = std::fs::read_to_string(body_file).map_err(|source| {
        AppError::new(
            ErrorCode::MarkdownConversionFailed,
            format!("Failed to read body file {}: {source}", body_file.display()),
        )
    })?;
    let converted = markdown_to_storage(&markdown)?;
    let space_id = client.resolve_space_id(space_key).await?;

    if !execute {
        let dry = create_dry_run(
            "POST",
            "/api/v2/pages",
            WriteTarget::Create {
                space_key: space_key.to_string(),
                space_id,
                parent_id,
            },
            title,
            &converted,
        );
        return Ok((true, dry));
    }

    let response = client
        .create_page(CreatePageRequest {
            space_id,
            title: title.to_string(),
            parent_id,
            storage_html: converted.storage_html,
        })
        .await?;
    Ok((false, response))
}

pub async fn update(
    _page_id: &str,
    _title: &str,
    _body_file: &Path,
    _execute: bool,
) -> Result<(bool, serde_json::Value), AppError> {
    Err(AppError::new(
        ErrorCode::InternalError,
        "page.update is unavailable in this incremental build.",
    ))
}
```

- [ ] **Step 4: Wire create command into CLI dispatch**

Edit the `PageCommand::Create` branch in `src/cli.rs`:

```rust
PageCommand::Create {
    space_key,
    title,
    body_file,
    parent_id,
    execute,
} => crate::commands::page::create(&space_key, &title, &body_file, parent_id, execute)
    .await
    .map(|(dry_run, data)| ("page.create", dry_run, data))
    .map_err(|error| ("page.create", error)),
```

- [ ] **Step 5: Run write-command tests to verify create tests pass**

Run:

```bash
cargo test --test write_commands
cargo test --test content_contract
cargo test --test http_contract
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/commands/page.rs src/cli.rs tests/write_commands.rs
git commit -m "feat: add page create command"
```

---

### Task 8: Page Update With Version Read

**Files:**
- Modify: `src/commands/page.rs`
- Modify: `src/cli.rs`
- Modify: `tests/write_commands.rs`

- [ ] **Step 1: Add page-update integration tests**

Append to `tests/write_commands.rs`:

```rust
#[tokio::test]
async fn page_update_without_execute_reads_version_and_does_not_put() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/pages/456"))
        .and(query_param("body-format", "storage"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "456",
            "status": "current",
            "title": "Existing Page",
            "spaceId": "987",
            "version": {"number": 7},
            "body": {"storage": {"value": "<p>Old</p>"}}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempdir().unwrap();
    let config = write_config(&dir, &server.uri());
    let body = dir.path().join("page.md");
    fs::write(&body, "# Updated Page\n\nBody").unwrap();

    let mut cmd = Command::cargo_bin("confluence-cli").unwrap();
    cmd.env("CONFLUENCE_CLI_CONFIG", config)
        .args([
            "page",
            "update",
            "--page-id",
            "456",
            "--title",
            "Updated Page",
            "--body-file",
            body.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"command\": \"page.update\""))
        .stdout(predicate::str::contains("\"dry_run\": true"))
        .stdout(predicate::str::contains("\"current_version\": 7"))
        .stdout(predicate::str::contains("\"next_version\": 8"));
}

#[tokio::test]
async fn page_update_execute_reads_version_then_puts_next_version() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/pages/456"))
        .and(query_param("body-format", "storage"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "456",
            "status": "current",
            "title": "Existing Page",
            "spaceId": "987",
            "version": {"number": 7},
            "body": {"storage": {"value": "<p>Old</p>"}}
        })))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/pages/456"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "456",
            "title": "Updated Page",
            "version": {"number": 8}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempdir().unwrap();
    let config = write_config(&dir, &server.uri());
    let body = dir.path().join("page.md");
    fs::write(&body, "# Updated Page\n\nBody").unwrap();

    let mut cmd = Command::cargo_bin("confluence-cli").unwrap();
    cmd.env("CONFLUENCE_CLI_CONFIG", config)
        .args([
            "page",
            "update",
            "--page-id",
            "456",
            "--title",
            "Updated Page",
            "--body-file",
            body.to_str().unwrap(),
            "--execute",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"dry_run\": false"))
        .stdout(predicate::str::contains("\"id\": \"456\""));
}

#[tokio::test]
async fn page_update_conflict_returns_stable_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/pages/456"))
        .and(query_param("body-format", "storage"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "456",
            "status": "current",
            "title": "Existing Page",
            "version": {"number": 7}
        })))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/pages/456"))
        .respond_with(ResponseTemplate::new(409).set_body_json(json!({"message": "Conflict"})))
        .mount(&server)
        .await;
    let dir = tempdir().unwrap();
    let config = write_config(&dir, &server.uri());
    let body = dir.path().join("page.md");
    fs::write(&body, "# Updated Page\n\nBody").unwrap();

    let mut cmd = Command::cargo_bin("confluence-cli").unwrap();
    cmd.env("CONFLUENCE_CLI_CONFIG", config)
        .args([
            "page",
            "update",
            "--page-id",
            "456",
            "--title",
            "Updated Page",
            "--body-file",
            body.to_str().unwrap(),
            "--execute",
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"code\": \"confluence_version_conflict\""));
}
```

- [ ] **Step 2: Run write-command tests to verify update tests fail**

Run:

```bash
cargo test --test write_commands
```

Expected: FAIL because page update still returns an internal error.

- [ ] **Step 3: Implement page update**

Replace the `update` function in `src/commands/page.rs`:

```rust
pub async fn update(
    page_id: &str,
    title: &str,
    body_file: &Path,
    execute: bool,
) -> Result<(bool, serde_json::Value), AppError> {
    let config = load_default_config()?;
    let client = ConfluenceClient::new(config)?;
    let markdown = std::fs::read_to_string(body_file).map_err(|source| {
        AppError::new(
            ErrorCode::MarkdownConversionFailed,
            format!("Failed to read body file {}: {source}", body_file.display()),
        )
    })?;
    let converted = markdown_to_storage(&markdown)?;
    let current = client.get_page(page_id).await?;
    let current_version = current
        .version
        .as_ref()
        .map(|version| version.number)
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::ConfluenceValidationFailed,
                "Confluence page response did not include a version number.",
            )
        })?;
    let next_version = current_version + 1;

    if !execute {
        let dry = create_dry_run(
            "PUT",
            format!("/api/v2/pages/{page_id}"),
            WriteTarget::Update {
                page_id: page_id.to_string(),
                current_version,
                next_version,
            },
            title,
            &converted,
        );
        return Ok((true, dry));
    }

    let response = client
        .update_page(UpdatePageRequest {
            page_id: page_id.to_string(),
            title: title.to_string(),
            next_version,
            storage_html: converted.storage_html,
        })
        .await?;
    Ok((false, response))
}
```

- [ ] **Step 4: Wire update command into CLI dispatch**

Edit the `PageCommand::Update` branch in `src/cli.rs`:

```rust
PageCommand::Update {
    page_id,
    title,
    body_file,
    execute,
} => crate::commands::page::update(&page_id, &title, &body_file, execute)
    .await
    .map(|(dry_run, data)| ("page.update", dry_run, data))
    .map_err(|error| ("page.update", error)),
```

- [ ] **Step 5: Run write-command tests to verify update tests pass**

Run:

```bash
cargo test --test write_commands
cargo test --test http_contract
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/commands/page.rs src/cli.rs tests/write_commands.rs
git commit -m "feat: add page update command"
```

---

### Task 9: README, Example Config, and Skills Package

**Files:**
- Create: `README.md`
- Create: `examples/config.toml`
- Create: `skills/confluence-cli/SKILL.md`
- Create: `skills/confluence-cli/skill.json`

- [ ] **Step 1: Write documentation smoke checks**

Run:

```bash
test ! -f README.md
test ! -f examples/config.toml
test ! -f skills/confluence-cli/SKILL.md
test ! -f skills/confluence-cli/skill.json
```

Expected: PASS before creating these files.

- [ ] **Step 2: Create README**

Create `README.md`:

```markdown
# confluence-cli

`confluence-cli` is an Agent-friendly CLI for Confluence Cloud. It exposes a small command surface with stable JSON output and safe dry-run writes.

## Install From Source

```bash
cargo install --path .
```

For local development:

```bash
cargo build --release
```

## Configure

Run:

```bash
confluence-cli config init
```

The default config path is:

```text
~/.config/confluence-cli/config.toml
```

Override it with:

```bash
CONFLUENCE_CLI_CONFIG=/path/to/config.toml confluence-cli space list
```

The API token is stored in plaintext. The CLI sets `0600` permissions on Unix platforms. Use a dedicated Atlassian API token and keep the config file out of source control.

## Commands

```bash
confluence-cli space list
confluence-cli search --query "deploy"
confluence-cli search --cql 'space = ENG and text ~ "deploy"'
confluence-cli page get --page-id 123456
confluence-cli page create --space-key ENG --title "New Page" --body-file page.md
confluence-cli page create --space-key ENG --title "New Page" --body-file page.md --execute
confluence-cli page update --page-id 123456 --title "Updated Page" --body-file page.md
confluence-cli page update --page-id 123456 --title "Updated Page" --body-file page.md --execute
```

All commands print JSON. Write commands are dry-run by default. A real create or update only happens when `--execute` is present.

## Agent Safety

Agents should run write commands without `--execute` first, inspect the returned JSON, and ask the user before executing the write. Updates require `--page-id`; the CLI does not update pages by title.

## Skills Package

`confluence-cli config init` asks whether to install the companion Skills package after saving your config. Press Enter to install it, or enter `n` to skip.

To install or reinstall it manually:

```bash
npx skills add laipz8200/confluence-cli --skill confluence-cli
```
```

- [ ] **Step 3: Create sample config**

Create `examples/config.toml`:

```toml
site_url = "https://example.atlassian.net/wiki"
email = "user@example.com"
api_token = "replace-with-atlassian-api-token"
default_space = "ENG"
```

- [ ] **Step 4: Create Skills manifest**

Create `skills/confluence-cli/skill.json`:

```json
{
  "name": "confluence-cli",
  "version": "0.1.0",
  "description": "Use confluence-cli to search, read, create, and update Confluence Cloud pages with safe dry-run writes.",
  "entry": "SKILL.md"
}
```

- [ ] **Step 5: Create Skills instructions**

Create `skills/confluence-cli/SKILL.md`:

```markdown
---
name: confluence-cli
description: Use when an Agent needs to search Confluence, read pages, create pages, or update pages through the confluence-cli binary.
---

# Confluence CLI Skill

Use `confluence-cli` for Confluence Cloud work. Treat the CLI as the only supported interface. Do not call Confluence REST APIs directly from this skill.

## Prerequisite Check

Before using the CLI, run:

```bash
confluence-cli --version
```

If the command is missing, tell the user to install it from the repository:

```bash
cargo install --path .
```

If configuration is missing, ask the user to run:

```bash
confluence-cli config init
```

## Read Commands

List spaces:

```bash
confluence-cli space list
```

Search with simple text:

```bash
confluence-cli search --query "deploy"
```

Search with raw CQL:

```bash
confluence-cli search --cql 'space = ENG and text ~ "deploy"'
```

Read a page:

```bash
confluence-cli page get --page-id 123456
```

## Write Commands

Write commands are dry-run by default. Always run the dry-run first.

Create dry-run:

```bash
confluence-cli page create --space-key ENG --title "New Page" --body-file page.md
```

Update dry-run:

```bash
confluence-cli page update --page-id 123456 --title "Updated Page" --body-file page.md
```

Only add `--execute` after the user explicitly approves the write.

Create execute:

```bash
confluence-cli page create --space-key ENG --title "New Page" --body-file page.md --execute
```

Update execute:

```bash
confluence-cli page update --page-id 123456 --title "Updated Page" --body-file page.md --execute
```

## Output Rules

Every command prints JSON. Check:

- `ok`: command success flag
- `command`: stable command name
- `dry_run`: true for dry-run write responses
- `data`: command result
- `error.code`: stable failure code when `ok` is false

## Safety Rules

- Do not print API tokens.
- Do not add `--execute` unless the user explicitly asks for the write to happen.
- Do not update pages by title.
- Do not create direct Confluence REST calls to bypass the CLI.
- If `ok` is false, report `error.code` and `error.message` to the user.
```

- [ ] **Step 6: Verify documentation files**

Run:

```bash
rg -n "api_token|--execute|dry-run|npx skills|confluence-cli page update" README.md skills/confluence-cli/SKILL.md examples/config.toml
```

Expected: PASS with matches in README, skill, and example config.

- [ ] **Step 7: Commit**

```bash
git add README.md examples/config.toml skills/confluence-cli/SKILL.md skills/confluence-cli/skill.json
git commit -m "docs: add README and Confluence skill"
```

---

### Task 10: Final Verification and Cleanup

**Files:**
- Modify: only files needed to fix failures found by verification commands.

- [ ] **Step 1: Format the Rust code**

Run:

```bash
cargo fmt --all
```

Expected: command exits 0.

- [ ] **Step 2: Run the full Rust test suite**

Run:

```bash
cargo test
```

Expected: PASS for all unit and integration tests.

- [ ] **Step 3: Run clippy with warnings denied**

Run:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: PASS with no warnings.

- [ ] **Step 4: Verify command help and JSON error behavior**

Run:

```bash
cargo run -- --help
CONFLUENCE_CLI_CONFIG=/tmp/missing-confluence-cli.toml cargo run -- space list
```

Expected: first command exits 0 and lists `config`, `space`, `search`, and `page`; second command exits non-zero and prints JSON containing `"ok": false` and `"code": "config_not_found"`.

- [ ] **Step 5: Scan for accidental secret output**

Run:

```bash
rg -n "token-value|replace-with-atlassian-api-token|Authorization|Basic " src tests README.md skills examples
```

Expected: matches are limited to tests, sample config, auth implementation, and documentation examples. No command output snapshot should expose a real token.

- [ ] **Step 6: Check git status**

Run:

```bash
git status --short
```

Expected: only intentional verification fixes are present. Commit them if any files changed.

- [ ] **Step 7: Commit verification fixes if needed**

If verification changed files, run:

```bash
git add <changed-files>
git commit -m "chore: finalize first CLI implementation"
```

If no files changed, do not create an empty commit.

---

## Self-Review Checklist

- Spec coverage: Tasks cover CLI scaffold, config, auth, JSON envelope, stable error codes, Confluence v2 pages, v2 spaces, v1 search, Markdown conversion, dry-run writes, page create, page update with version read, README, example config, and generic Skills package.
- Scope check: The plan does not implement Data Center, multiple profiles, attachments, comments, labels, permissions, macros, page tree management, keychain storage, or arbitrary REST request passthrough.
- Safety check: Write commands are dry-run unless `--execute` is present, and the Skills package requires user approval before real writes.
- Type consistency: Command names use `config.init`, `space.list`, `search`, `page.get`, `page.create`, and `page.update` consistently.
