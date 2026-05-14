use crate::content::{markdown_to_storage, storage_to_storage, ConvertedContent};
use crate::error::{AppError, ErrorCode};
use clap::ValueEnum;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum BodyRepresentation {
    Markdown,
    Storage,
}

pub fn read_body_file(
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
