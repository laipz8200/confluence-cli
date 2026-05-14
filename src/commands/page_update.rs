use crate::client::UpdatePageRequest;
use crate::command_context::CommandContext;
use crate::commands::page_body::{read_body_file, BodyRepresentation};
use crate::commands::{CommandOutput, CommandResult};
use crate::dry_run::{create_dry_run, WriteTarget};
use crate::error::{AppError, ErrorCode};
use clap::Args;
use std::path::PathBuf;

pub const COMMAND: &str = "page.update";

#[derive(Debug, Args)]
pub struct PageUpdateArgs {
    #[arg(long)]
    page_id: String,
    #[arg(long)]
    title: String,
    #[arg(long)]
    body_file: PathBuf,
    #[arg(long, value_enum)]
    body_representation: Option<BodyRepresentation>,
    #[arg(long)]
    execute: bool,
}

pub async fn run(args: PageUpdateArgs, ctx: CommandContext) -> CommandResult {
    let converted = read_body_file(&args.body_file, args.body_representation)?;
    let page = ctx.client().get_page(&args.page_id).await?;
    let current_version = page.version.map(|version| version.number).ok_or_else(|| {
        AppError::new(
            ErrorCode::ConfluenceValidationFailed,
            "Confluence page response did not include a version number.",
        )
    })?;
    let next_version = current_version + 1;

    if !args.execute {
        return Ok(CommandOutput::new(
            COMMAND,
            true,
            create_dry_run(
                "PUT",
                format!("/api/v2/pages/{}", args.page_id),
                WriteTarget::Update {
                    page_id: args.page_id,
                    current_version,
                    next_version,
                },
                &args.title,
                &converted,
            ),
        ));
    }

    let response = ctx
        .client()
        .update_page(UpdatePageRequest {
            page_id: args.page_id,
            title: args.title,
            next_version,
            storage_html: converted.storage_html,
        })
        .await?;

    Ok(CommandOutput::new(COMMAND, false, response))
}
