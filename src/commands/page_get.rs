use crate::command_context::CommandContext;
use crate::commands::{CommandOutput, CommandResult};
use crate::error::{AppError, ErrorCode};
use clap::Args;

pub const COMMAND: &str = "page.get";

#[derive(Debug, Args)]
pub struct PageGetArgs {
    #[arg(long)]
    page_id: String,
}

pub async fn run(args: PageGetArgs, ctx: CommandContext) -> CommandResult {
    let page = ctx.client().get_page(&args.page_id).await?;
    let data = serde_json::to_value(page).map_err(|source| {
        AppError::new(
            ErrorCode::InternalError,
            format!("Failed to serialize page JSON: {source}"),
        )
    })?;

    Ok(CommandOutput::new(COMMAND, false, data))
}
