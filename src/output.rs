use crate::error::{AppError, ErrorBody, ErrorCode};
use serde::Serialize;
use serde_json::{json, Value};
use std::io::{self, Write};

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
            ErrorCode::InternalError,
            format!("Failed to serialize JSON output: {source}"),
        )
    })?;
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(text.as_bytes())
        .and_then(|_| stdout.write_all(b"\n"))
        .map_err(|source| {
            AppError::new(
                ErrorCode::InternalError,
                format!("Failed to write JSON output: {source}"),
            )
        })?;
    Ok(())
}
