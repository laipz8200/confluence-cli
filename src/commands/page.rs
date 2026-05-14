use crate::client::{ConfluenceClient, CreatePageRequest, UpdatePageRequest};
use crate::config::load_default_config;
use crate::content::{markdown_to_storage, storage_to_storage, ConvertedContent};
use crate::dry_run::{create_dry_run, WriteTarget};
use crate::error::{AppError, ErrorCode};
use clap::ValueEnum;
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum BodyRepresentation {
    Markdown,
    Storage,
}

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
    body_representation: Option<BodyRepresentation>,
    parent_id: Option<String>,
    execute: bool,
) -> Result<(bool, Value), AppError> {
    let config = load_default_config()?;
    let client = ConfluenceClient::new(config)?;
    let converted = read_body_file(body_file, body_representation)?;
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
    body_representation: Option<BodyRepresentation>,
    execute: bool,
) -> Result<(bool, Value), AppError> {
    let config = load_default_config()?;
    let client = ConfluenceClient::new(config)?;
    let converted = read_body_file(body_file, body_representation)?;
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

fn read_body_file(
    body_file: &Path,
    body_representation: Option<BodyRepresentation>,
) -> Result<ConvertedContent, AppError> {
    let body = std::fs::read_to_string(body_file).map_err(|source| {
        AppError::new(
            ErrorCode::MarkdownConversionFailed,
            format!("Failed to read body file: {source}"),
        )
    })?;

    match body_representation.unwrap_or_else(|| infer_body_representation(body_file)) {
        BodyRepresentation::Markdown => markdown_to_storage(&body),
        BodyRepresentation::Storage => storage_to_storage(&body),
    }
}

fn infer_body_representation(body_file: &Path) -> BodyRepresentation {
    let Some(file_name) = body_file.file_name().and_then(|name| name.to_str()) else {
        return BodyRepresentation::Markdown;
    };
    let file_name = file_name.to_ascii_lowercase();

    if file_name.ends_with(".storage.xml")
        || file_name.ends_with(".storage")
        || file_name.ends_with(".xml")
    {
        BodyRepresentation::Storage
    } else {
        BodyRepresentation::Markdown
    }
}
