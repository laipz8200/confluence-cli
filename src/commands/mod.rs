use crate::command_context::CommandContext;
use crate::error::AppError;
use clap::Subcommand;

mod config_init;
mod page_body;
mod page_create;
mod page_get;
mod page_update;
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
    Create(page_create::PageCreateArgs),
    Update(page_update::PageUpdateArgs),
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
            PageCommand::Create(args) => {
                let ctx = load_context(page_create::COMMAND)?;
                page_create::run(args, ctx)
                    .await
                    .map_err(to_failure(page_create::COMMAND))
            }
            PageCommand::Update(args) => {
                let ctx = load_context(page_update::COMMAND)?;
                page_update::run(args, ctx)
                    .await
                    .map_err(to_failure(page_update::COMMAND))
            }
        },
    }
}

fn load_context(command: &'static str) -> Result<CommandContext, CommandFailure> {
    CommandContext::load().map_err(|error| CommandFailure::new(command, error))
}

fn to_failure(command: &'static str) -> impl FnOnce(AppError) -> CommandFailure {
    move |error| CommandFailure::new(command, error)
}
