use crate::command_context::CommandContext;
use crate::error::AppError;
use clap::Subcommand;

mod config_init;
pub mod page;
mod page_get;
mod search;
mod space_list;

#[derive(Debug)]
pub struct CommandOutput {
    pub command: &'static str,
    pub dry_run: bool,
    pub data: serde_json::Value,
}

impl CommandOutput {
    pub fn new(command: &'static str, dry_run: bool, data: serde_json::Value) -> Self {
        Self {
            command,
            dry_run,
            data,
        }
    }
}

#[derive(Debug)]
pub struct CommandFailure {
    pub command: &'static str,
    pub error: AppError,
}

impl CommandFailure {
    pub fn new(command: &'static str, error: AppError) -> Self {
        Self { command, error }
    }
}

pub type CommandResult = Result<CommandOutput, AppError>;
pub type DispatchResult = Result<CommandOutput, CommandFailure>;

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
    Search(search::SearchArgs),
    Page {
        #[command(subcommand)]
        command: PageCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    Init(config_init::ConfigInitArgs),
}

#[derive(Debug, Subcommand)]
pub enum SpaceCommand {
    List(space_list::SpaceListArgs),
}

#[derive(Debug, Subcommand)]
pub enum PageCommand {
    Get(page_get::PageGetArgs),
    Create {
        #[arg(long)]
        space_key: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        body_file: std::path::PathBuf,
        #[arg(long, value_enum)]
        body_representation: Option<page::BodyRepresentation>,
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
        #[arg(long, value_enum)]
        body_representation: Option<page::BodyRepresentation>,
        #[arg(long)]
        execute: bool,
    },
}

pub async fn dispatch(command: Commands) -> DispatchResult {
    match command {
        Commands::Config {
            command: ConfigCommand::Init(args),
        } => config_init::run(args).map_err(to_failure(config_init::COMMAND)),
        Commands::Space {
            command: SpaceCommand::List(args),
        } => {
            let ctx = load_context(space_list::COMMAND)?;
            space_list::run(args, ctx)
                .await
                .map_err(to_failure(space_list::COMMAND))
        }
        Commands::Search(args) => {
            let cql = args.cql().map_err(to_failure(search::COMMAND))?;
            let ctx = load_context(search::COMMAND)?;
            search::run(cql, ctx)
                .await
                .map_err(to_failure(search::COMMAND))
        }
        Commands::Page { command } => match command {
            PageCommand::Get(args) => {
                let ctx = load_context(page_get::COMMAND)?;
                page_get::run(args, ctx)
                    .await
                    .map_err(to_failure(page_get::COMMAND))
            }
            PageCommand::Create {
                space_key,
                title,
                body_file,
                body_representation,
                parent_id,
                execute,
            } => page::create(
                &space_key,
                &title,
                &body_file,
                body_representation,
                parent_id,
                execute,
            )
            .await
            .map(|(dry_run, data)| CommandOutput::new("page.create", dry_run, data))
            .map_err(to_failure("page.create")),
            PageCommand::Update {
                page_id,
                title,
                body_file,
                body_representation,
                execute,
            } => page::update(
                &page_id,
                &title,
                &body_file,
                body_representation,
                execute,
            )
            .await
            .map(|(dry_run, data)| CommandOutput::new("page.update", dry_run, data))
            .map_err(to_failure("page.update")),
        },
    }
}

fn load_context(command: &'static str) -> Result<CommandContext, CommandFailure> {
    CommandContext::load().map_err(|error| CommandFailure::new(command, error))
}

fn to_failure(command: &'static str) -> impl FnOnce(AppError) -> CommandFailure {
    move |error| CommandFailure::new(command, error)
}
