use crate::command_context::CommandContext;
use crate::commands::{CommandOutput, CommandResult};
use crate::error::{AppError, ErrorCode};
use clap::Args;
use serde_json::json;

pub const COMMAND: &str = "search";

#[derive(Debug, Args)]
pub struct SearchArgs {
    #[arg(long)]
    query: Option<String>,
    #[arg(long)]
    cql: Option<String>,
}

impl SearchArgs {
    pub fn cql(self) -> Result<String, AppError> {
        match (self.query, self.cql) {
            (Some(query), None) => Ok(format!("text ~ \"{}\"", escape_cql_text(&query))),
            (None, Some(cql)) => Ok(cql),
            _ => Err(AppError::new(
                ErrorCode::ConfluenceValidationFailed,
                "Provide exactly one of --query or --cql.",
            )),
        }
    }
}

pub async fn run(cql: String, ctx: CommandContext) -> CommandResult {
    let result = ctx.client().search(&cql).await?;

    Ok(CommandOutput::new(
        COMMAND,
        false,
        json!({ "cql": cql, "result": result }),
    ))
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
