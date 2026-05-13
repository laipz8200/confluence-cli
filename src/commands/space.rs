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
