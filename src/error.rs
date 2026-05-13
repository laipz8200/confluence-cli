use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    ConfigNotFound,
    ConfigInvalid,
    AuthFailed,
    PermissionDenied,
    NotFound,
    RateLimited,
    ConfluenceValidationFailed,
    ConfluenceVersionConflict,
    NetworkError,
    MarkdownConversionFailed,
    UnsupportedMarkdown,
    InternalError,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::ConfigNotFound => "config_not_found",
            ErrorCode::ConfigInvalid => "config_invalid",
            ErrorCode::AuthFailed => "auth_failed",
            ErrorCode::PermissionDenied => "permission_denied",
            ErrorCode::NotFound => "not_found",
            ErrorCode::RateLimited => "rate_limited",
            ErrorCode::ConfluenceValidationFailed => "confluence_validation_failed",
            ErrorCode::ConfluenceVersionConflict => "confluence_version_conflict",
            ErrorCode::NetworkError => "network_error",
            ErrorCode::MarkdownConversionFailed => "markdown_conversion_failed",
            ErrorCode::UnsupportedMarkdown => "unsupported_markdown",
            ErrorCode::InternalError => "internal_error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub retryable: bool,
    pub details: Value,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: false,
            details: Value::Object(Map::new()),
        }
    }

    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = redact_value(details);
        self
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for AppError {}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
    pub retryable: bool,
    pub details: Value,
}

impl From<&AppError> for ErrorBody {
    fn from(value: &AppError) -> Self {
        Self {
            code: value.code.as_str(),
            message: value.message.clone(),
            retryable: value.retryable,
            details: value.details.clone(),
        }
    }
}

pub fn redact_value(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let lower = key.to_ascii_lowercase();
                    if lower.contains("authorization")
                        || lower.contains("api_token")
                        || lower == "token"
                    {
                        (key, Value::String("[redacted]".to_string()))
                    } else {
                        (key, redact_value(value))
                    }
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(redact_value).collect()),
        other => other,
    }
}
