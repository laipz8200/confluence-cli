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
