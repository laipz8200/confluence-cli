use confluence_cli::content::markdown_to_storage;
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
