use confluence_cli::content::{markdown_to_storage, storage_to_storage};
use confluence_cli::dry_run::{create_dry_run, WriteTarget};
use pretty_assertions::assert_eq;

#[test]
fn markdown_conversion_supports_common_subset() {
    let converted =
        markdown_to_storage("# Title\n\nA **bold** [link](https://example.com).\n\n- one\n- two\n")
            .unwrap();

    assert!(converted.storage_html.contains("<h1>Title</h1>"));
    assert!(converted.storage_html.contains("<strong>bold</strong>"));
    assert!(converted
        .storage_html
        .contains("<a href=\"https://example.com\">link</a>"));
    assert!(converted.storage_html.contains("<ul>"));
    assert_eq!(converted.headings, vec!["Title"]);
}

#[test]
fn unsupported_image_returns_stable_error_code() {
    let error = markdown_to_storage("![alt](image.png)").unwrap_err();

    assert_eq!(error.code.as_str(), "unsupported_markdown");
}

#[test]
fn unsafe_link_scheme_returns_stable_error_code() {
    let error = markdown_to_storage("[bad](javascript:alert(1))").unwrap_err();

    assert_eq!(error.code.as_str(), "unsupported_markdown");
}

#[test]
fn safe_link_destinations_are_accepted() {
    let converted = markdown_to_storage(
        "[https](https://example.com) [relative](/spaces/ENG/pages/123) [parent](../page) [anchor](#section) [mail](mailto:user@example.com)",
    )
    .unwrap();

    assert!(converted
        .storage_html
        .contains("<a href=\"https://example.com\">https</a>"));
    assert!(converted
        .storage_html
        .contains("<a href=\"/spaces/ENG/pages/123\">relative</a>"));
    assert!(converted
        .storage_html
        .contains("<a href=\"../page\">parent</a>"));
    assert!(converted
        .storage_html
        .contains("<a href=\"#section\">anchor</a>"));
    assert!(converted
        .storage_html
        .contains("<a href=\"mailto:user@example.com\">mail</a>"));
}

#[test]
fn heading_extraction_uses_enabled_inline_extensions() {
    let converted = markdown_to_storage("# ~~Gone~~ now").unwrap();

    assert_eq!(converted.headings, vec!["Gone now"]);
    assert!(!converted.headings[0].contains("~~"));
}

#[test]
fn storage_representation_passes_confluence_storage_through() {
    let storage = r#"<ac:structured-macro ac:name="recently-updated" ac:schema-version="1"><ac:parameter ac:name="max">5</ac:parameter></ac:structured-macro>"#;

    let converted = storage_to_storage(storage).unwrap();

    assert_eq!(converted.storage_html, storage);
    assert_eq!(converted.source_representation, "storage");
    assert_eq!(converted.source_bytes, storage.len());
    assert_eq!(converted.storage_html_bytes, storage.len());
    assert!(converted.headings.is_empty());
}

#[test]
fn dry_run_summary_excludes_full_body() {
    let converted = markdown_to_storage("# Title\n\nBody").unwrap();
    let summary = create_dry_run(
        "POST",
        "/api/v2/pages",
        WriteTarget::Create {
            space_key: "ENG".to_string(),
            space_id: "987".to_string(),
            parent_id: Some("123".to_string()),
        },
        "Title",
        &converted,
    );
    let text = serde_json::to_string(&summary).unwrap();

    assert!(text.contains("\"method\":\"POST\""));
    assert!(text.contains("\"space_key\":\"ENG\""));
    assert!(text.contains("\"storage_html_bytes\""));
    assert!(!text.contains("<h1>Title</h1>"));
}

#[test]
fn dry_run_update_summary_excludes_full_body() {
    let converted = markdown_to_storage("# Title\n\nBody").unwrap();
    let summary = create_dry_run(
        "PUT",
        "/api/v2/pages/123",
        WriteTarget::Update {
            page_id: "123".to_string(),
            current_version: 7,
            next_version: 8,
        },
        "Title",
        &converted,
    );
    let text = serde_json::to_string(&summary).unwrap();

    assert!(text.contains("\"method\":\"PUT\""));
    assert!(text.contains("\"page_id\":\"123\""));
    assert!(text.contains("\"current_version\":7"));
    assert!(text.contains("\"next_version\":8"));
    assert!(text.contains("\"storage_html_bytes\""));
    assert!(!text.contains("<h1>Title</h1>"));
}
