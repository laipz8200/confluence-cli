use confluence_cli::error::{AppError, ErrorCode};
use confluence_cli::output::{error_json, success_json};
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn success_envelope_has_stable_shape() {
    let value = success_json("page.update", true, json!({"page_id": "123"}));

    assert_eq!(
        value,
        json!({
            "ok": true,
            "command": "page.update",
            "dry_run": true,
            "data": {"page_id": "123"}
        })
    );
}

#[test]
fn error_envelope_has_stable_shape() {
    let error = AppError::new(
        ErrorCode::ConfluenceVersionConflict,
        "Page was updated by someone else. Fetch the latest version and retry.",
    )
    .with_retryable(true)
    .with_details(json!({"status": 409}));

    let value = error_json("page.update", &error);

    assert_eq!(
        value,
        json!({
            "ok": false,
            "command": "page.update",
            "error": {
                "code": "confluence_version_conflict",
                "message": "Page was updated by someone else. Fetch the latest version and retry.",
                "retryable": true,
                "details": {"status": 409}
            }
        })
    );
}

#[test]
fn token_like_details_are_redacted() {
    let error =
        AppError::new(ErrorCode::AuthFailed, "Authentication failed.").with_details(json!({
            "Authorization": "Basic abc123",
            "api_token": "api-token-secret",
            "access_token": "access-secret",
            "refresh_token": "refresh-secret",
            "bearer_token": "bearer-secret",
            "apiToken": "camel-secret",
            "secret": "secret-value",
            "nested": {
                "token": "nested-secret",
                "items": [
                    {"Authorization": "Bearer array-secret"},
                    {"apiToken": "array-camel-secret"},
                    {"safe": "visible"}
                ]
            }
        }));

    let value = error_json("space.list", &error);
    let text = serde_json::to_string(&value).unwrap();

    assert!(!text.contains("abc123"));
    assert!(!text.contains("api-token-secret"));
    assert!(!text.contains("access-secret"));
    assert!(!text.contains("refresh-secret"));
    assert!(!text.contains("bearer-secret"));
    assert!(!text.contains("camel-secret"));
    assert!(!text.contains("secret-value"));
    assert!(!text.contains("nested-secret"));
    assert!(!text.contains("array-secret"));
    assert!(!text.contains("array-camel-secret"));
    assert!(text.contains("visible"));
    assert!(text.contains("[redacted]"));
}
