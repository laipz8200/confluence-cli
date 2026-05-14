use crate::command_context::CommandContext;
use crate::commands::{CommandOutput, CommandResult};
use clap::Args;
use serde_json::json;

pub const COMMAND: &str = "space.list";

#[derive(Debug, Args)]
pub struct SpaceListArgs {}

pub async fn run(_args: SpaceListArgs, ctx: CommandContext) -> CommandResult {
    let spaces = ctx.client().list_spaces().await?;

    Ok(CommandOutput::new(
        COMMAND,
        false,
        json!({ "spaces": spaces }),
    ))
}
