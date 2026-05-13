use crate::client::ConfluenceClient;
use crate::config::load_default_config;
use crate::error::{AppError, ErrorCode};
use serde_json::json;

pub async fn run(
    query: Option<String>,
    cql: Option<String>,
) -> Result<serde_json::Value, AppError> {
    let cql = match (query, cql) {
        (Some(query), None) => format!("text ~ \"{}\"", escape_cql_text(&query)),
        (None, Some(cql)) => cql,
        _ => {
            return Err(AppError::new(
                ErrorCode::ConfluenceValidationFailed,
                "Provide exactly one of --query or --cql.",
            ));
        }
    };

    let config = load_default_config()?;
    let client = ConfluenceClient::new(config)?;
    let result = client.search(&cql).await?;

    Ok(json!({ "cql": cql, "result": result }))
}

fn escape_cql_text(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    #[test]
    fn escape_cql_text_escapes_backslashes_and_quotes() {
        assert_eq!(
            super::escape_cql_text(r#"deploy \ "guide""#),
            r#"deploy \\ \"guide\""#
        );
    }
}
