use crate::client::CreatePageRequest;
use crate::command_context::CommandContext;
use crate::commands::page_body::{read_body_file, BodyRepresentation};
use crate::commands::{CommandOutput, CommandResult};
use crate::dry_run::{create_dry_run, WriteTarget};
use clap::Args;
use std::path::PathBuf;

pub const COMMAND: &str = "page.create";

#[derive(Debug, Args)]
pub struct PageCreateArgs {
    #[arg(long)]
    space_key: String,
    #[arg(long)]
    title: String,
    #[arg(long)]
    body_file: PathBuf,
    #[arg(long, value_enum)]
    body_representation: Option<BodyRepresentation>,
    #[arg(long)]
    parent_id: Option<String>,
    #[arg(long)]
    execute: bool,
}

pub async fn run(args: PageCreateArgs, ctx: CommandContext) -> CommandResult {
    let converted = read_body_file(&args.body_file, args.body_representation)?;
    let space_id = ctx.client().resolve_space_id(&args.space_key).await?;

    if !args.execute {
        return Ok(CommandOutput::new(
            COMMAND,
            true,
            create_dry_run(
                "POST",
                "/api/v2/pages",
                WriteTarget::Create {
                    space_key: args.space_key,
                    space_id,
                    parent_id: args.parent_id,
                },
                &args.title,
                &converted,
            ),
        ));
    }

    let response = ctx
        .client()
        .create_page(CreatePageRequest {
            space_id,
            title: args.title,
            parent_id: args.parent_id,
            storage_html: converted.storage_html,
        })
        .await?;

    Ok(CommandOutput::new(COMMAND, false, response))
}
