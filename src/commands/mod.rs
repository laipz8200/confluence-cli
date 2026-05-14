use crate::error::AppError;
use clap::Subcommand;

pub mod config;
pub mod page;
pub mod search;
pub mod space;

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
    Search {
        #[arg(long)]
        query: Option<String>,
        #[arg(long)]
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
            command: ConfigCommand::Init,
        } => config::init()
            .map(|data| CommandOutput::new("config.init", false, data))
            .map_err(|error| CommandFailure::new("config.init", error)),
        Commands::Space {
            command: SpaceCommand::List,
        } => space::list()
            .await
            .map(|data| CommandOutput::new("space.list", false, data))
            .map_err(|error| CommandFailure::new("space.list", error)),
        Commands::Search { query, cql } => search::run(query, cql)
            .await
            .map(|data| CommandOutput::new("search", false, data))
            .map_err(|error| CommandFailure::new("search", error)),
        Commands::Page { command } => match command {
            PageCommand::Get { page_id } => page::get(&page_id)
                .await
                .map(|data| CommandOutput::new("page.get", false, data))
                .map_err(|error| CommandFailure::new("page.get", error)),
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
            .map_err(|error| CommandFailure::new("page.create", error)),
            PageCommand::Update {
                page_id,
                title,
                body_file,
                body_representation,
                execute,
            } => page::update(&page_id, &title, &body_file, body_representation, execute)
                .await
                .map(|(dry_run, data)| CommandOutput::new("page.update", dry_run, data))
                .map_err(|error| CommandFailure::new("page.update", error)),
        },
    }
}
