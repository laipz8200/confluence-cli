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
