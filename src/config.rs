use crate::error::{AppError, ErrorCode};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    pub site_url: String,
    pub email: String,
    pub api_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_space: Option<String>,
}

impl Config {
    pub fn validate(mut self) -> Result<Self, AppError> {
        self.site_url = self.site_url.trim_end_matches('/').to_string();
        self.default_space = self
            .default_space
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if self.site_url.is_empty() || self.email.is_empty() || self.api_token.is_empty() {
            return Err(AppError::new(
                ErrorCode::ConfigInvalid,
                "Config must include site_url, email, and api_token.",
            ));
        }
        let site_url = Url::parse(&self.site_url).map_err(|source| {
            AppError::new(
                ErrorCode::ConfigInvalid,
                format!("Config site_url must be a valid URL: {source}"),
            )
        })?;
        if site_url.scheme() != "https" && !is_loopback_http_url(&site_url) {
            return Err(AppError::new(
                ErrorCode::ConfigInvalid,
                "Config site_url must use https:// unless it points to a loopback HTTP test server.",
            ));
        }
        Ok(self)
    }
}

fn is_loopback_http_url(site_url: &Url) -> bool {
    if site_url.scheme() != "http" {
        return false;
    }

    match site_url.host_str() {
        Some("localhost") => true,
        Some(host) => host.parse::<IpAddr>().is_ok_and(|addr| addr.is_loopback()),
        None => false,
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
        AppError::new(
            code,
            format!("Failed to read config at {}.", path.display()),
        )
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
                format!(
                    "Failed to create config directory {}: {source}",
                    parent.display()
                ),
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
    let mut file = secured_config_file(path).map_err(|source| {
        AppError::new(
            ErrorCode::ConfigInvalid,
            format!("Failed to open config at {}: {source}", path.display()),
        )
    })?;
    file.set_len(0).map_err(|source| {
        AppError::new(
            ErrorCode::ConfigInvalid,
            format!("Failed to truncate config at {}: {source}", path.display()),
        )
    })?;
    file.write_all(text.as_bytes()).map_err(|source| {
        AppError::new(
            ErrorCode::ConfigInvalid,
            format!("Failed to write config at {}: {source}", path.display()),
        )
    })?;
    file.flush().map_err(|source| {
        AppError::new(
            ErrorCode::ConfigInvalid,
            format!("Failed to flush config at {}: {source}", path.display()),
        )
    })?;
    Ok(())
}

#[cfg(unix)]
fn secured_config_file(path: &Path) -> std::io::Result<File> {
    match create_new_config_file(path) {
        Ok(file) => {
            set_owner_only_file_permissions(&file)?;
            Ok(file)
        }
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
            let file = OpenOptions::new().write(true).open(path)?;
            set_owner_only_file_permissions(&file)?;
            Ok(file)
        }
        Err(source) => Err(source),
    }
}

#[cfg(unix)]
fn create_new_config_file(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
}

#[cfg(unix)]
fn set_owner_only_file_permissions(file: &File) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    file.set_permissions(std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn secured_config_file(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().create(true).write(true).open(path)
}
