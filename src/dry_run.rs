use crate::content::ConvertedContent;
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WriteTarget {
    Create {
        space_key: String,
        space_id: String,
        parent_id: Option<String>,
    },
    Update {
        page_id: String,
        current_version: u64,
        next_version: u64,
    },
}

pub fn create_dry_run(
    method: &'static str,
    endpoint: impl Into<String>,
    target: WriteTarget,
    title: &str,
    content: &ConvertedContent,
) -> Value {
    json!({
        "method": method,
        "endpoint": endpoint.into(),
        "target": target,
        "title": title,
        "body": {
            "format": "storage",
            "summary": {
                "markdown_bytes": content.markdown_bytes,
                "storage_html_bytes": content.storage_html_bytes,
                "headings": content.headings,
            }
        },
        "payload_preview": {
            "status": "current",
            "title": title,
            "body": {
                "representation": "storage",
                "value": "[omitted]"
            }
        }
    })
}
