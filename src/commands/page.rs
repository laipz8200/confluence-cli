use crate::client::ConfluenceClient;
use crate::config::load_default_config;
use crate::error::{AppError, ErrorCode};

pub async fn get(page_id: &str) -> Result<serde_json::Value, AppError> {
    let config = load_default_config()?;
    let client = ConfluenceClient::new(config)?;
    let page = client.get_page(page_id).await?;

    serde_json::to_value(page).map_err(|source| {
        AppError::new(
            ErrorCode::InternalError,
            format!("Failed to serialize page JSON: {source}"),
        )
    })
}
