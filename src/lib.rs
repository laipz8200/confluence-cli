pub mod auth;
pub mod cli;
pub mod client;
pub mod command_context;
pub mod commands;
pub mod config;
pub mod content;
pub mod dry_run;
pub mod error;
pub mod output;

pub async fn run() -> i32 {
    cli::run().await
}
