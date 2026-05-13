use crate::client::{ConfluenceClient, CreatePageRequest, UpdatePageRequest};
use crate::config::load_default_config;
use crate::content::markdown_to_storage;
use crate::dry_run::{create_dry_run, WriteTarget};
use crate::error::{AppError, ErrorCode};
use serde_json::Value;
use std::path::Path;

pub async fn get(page_id: &str) -> Result<Value, AppError> {
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

pub async fn create(
    space_key: &str,
    title: &str,
    body_file: &Path,
    parent_id: Option<String>,
    execute: bool,
) -> Result<(bool, Value), AppError> {
    let config = load_default_config()?;
    let client = ConfluenceClient::new(config)?;
    let markdown = std::fs::read_to_string(body_file).map_err(|source| {
        AppError::new(
            ErrorCode::MarkdownConversionFailed,
            format!("Failed to read body file: {source}"),
        )
    })?;
    let converted = markdown_to_storage(&markdown)?;
    let space_id = client.resolve_space_id(space_key).await?;

    if !execute {
        return Ok((
            true,
            create_dry_run(
                "POST",
                "/api/v2/pages",
                WriteTarget::Create {
                    space_key: space_key.to_string(),
                    space_id,
                    parent_id,
                },
                title,
                &converted,
            ),
        ));
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
    page_id: &str,
    title: &str,
    body_file: &Path,
    execute: bool,
) -> Result<(bool, Value), AppError> {
    let config = load_default_config()?;
    let client = ConfluenceClient::new(config)?;
    let markdown = std::fs::read_to_string(body_file).map_err(|source| {
        AppError::new(
            ErrorCode::MarkdownConversionFailed,
            format!("Failed to read body file: {source}"),
        )
    })?;
    let converted = markdown_to_storage(&markdown)?;
    let page = client.get_page(page_id).await?;
    let current_version = page.version.map(|version| version.number).ok_or_else(|| {
        AppError::new(
            ErrorCode::ConfluenceValidationFailed,
            "Confluence page response did not include a version number.",
        )
    })?;
    let next_version = current_version + 1;

    if !execute {
        return Ok((
            true,
            create_dry_run(
                "PUT",
                format!("/api/v2/pages/{page_id}"),
                WriteTarget::Update {
                    page_id: page_id.to_string(),
                    current_version,
                    next_version,
                },
                title,
                &converted,
            ),
        ));
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
