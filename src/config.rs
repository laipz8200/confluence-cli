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
