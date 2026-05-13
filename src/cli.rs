use crate::error::{AppError, ErrorCode};
use crate::output::{error_json, print_json, success_json};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "confluence-cli")]
#[command(version)]
#[command(about = "Agent-friendly CLI for Confluence Cloud")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    Space {
        #[command(subcommand)]
        command: SpaceCommand,
    },
    Search {
        #[arg(long, conflicts_with = "cql")]
        query: Option<String>,
        #[arg(long, conflicts_with = "query")]
        cql: Option<String>,
    },
    Page {
        #[command(subcommand)]
        command: PageCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    Init,
}

#[derive(Debug, Subcommand)]
pub enum SpaceCommand {
    List,
}

#[derive(Debug, Subcommand)]
pub enum PageCommand {
    Get {
        #[arg(long)]
        page_id: String,
    },
    Create {
        #[arg(long)]
        space_key: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        body_file: std::path::PathBuf,
        #[arg(long)]
        parent_id: Option<String>,
        #[arg(long)]
        execute: bool,
    },
    Update {
        #[arg(long)]
        page_id: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        body_file: std::path::PathBuf,
        #[arg(long)]
        execute: bool,
    },
}

pub async fn run() -> i32 {
    let cli = Cli::parse();
    let result = dispatch(cli).await;

    match result {
        Ok((command, dry_run, data)) => match print_json(&success_json(command, dry_run, data)) {
            Ok(()) => 0,
            Err(error) => {
                let _ = print_json(&error_json(command, &error));
                1
            }
        },
        Err((command, error)) => {
            let fallback = if command.is_empty() {
                "unknown"
            } else {
                command
            };
            let _ = print_json(&error_json(fallback, &error));
            1
        }
    }
}

async fn dispatch(
    cli: Cli,
) -> Result<(&'static str, bool, serde_json::Value), (&'static str, AppError)> {
    match cli.command {
        Commands::Config {
            command: ConfigCommand::Init,
        } => crate::commands::config::init()
            .map(|data| ("config.init", false, data))
            .map_err(|error| ("config.init", error)),
        Commands::Space {
            command: SpaceCommand::List,
        } => crate::commands::space::list()
            .await
            .map(|data| ("space.list", false, data))
            .map_err(|error| ("space.list", error)),
        Commands::Search { query, cql } => crate::commands::search::run(query, cql)
            .await
            .map(|data| ("search", false, data))
            .map_err(|error| ("search", error)),
        Commands::Page { command } => match command {
            PageCommand::Get { page_id } => crate::commands::page::get(&page_id)
                .await
                .map(|data| ("page.get", false, data))
                .map_err(|error| ("page.get", error)),
            PageCommand::Create { .. } => Err((
                "page.create",
                AppError::new(
                    ErrorCode::InternalError,
                    "page.create is unavailable in this incremental build.",
                ),
            )),
            PageCommand::Update { .. } => Err((
                "page.update",
                AppError::new(
                    ErrorCode::InternalError,
                    "page.update is unavailable in this incremental build.",
                ),
            )),
        },
    }
}
