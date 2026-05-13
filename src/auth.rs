use crate::error::{AppError, ErrorCode};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use reqwest::header::{HeaderValue, AUTHORIZATION};

pub fn basic_auth_header(email: &str, api_token: &str) -> Result<HeaderValue, AppError> {
    let encoded = STANDARD.encode(format!("{email}:{api_token}"));
    HeaderValue::from_str(&format!("Basic {encoded}")).map_err(|source| {
        AppError::new(
            ErrorCode::ConfigInvalid,
            format!("Failed to build authorization header: {source}"),
        )
    })
}

pub fn auth_header_name() -> reqwest::header::HeaderName {
    AUTHORIZATION
}

pub fn redacted_token(_api_token: &str) -> String {
    "[redacted]".to_string()
}
