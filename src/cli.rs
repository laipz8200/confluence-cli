use crate::commands::{dispatch, Commands};
use crate::output::{error_json, print_json, print_text, success_json};
use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "confluence-cli")]
#[command(version)]
#[command(about = "Agent-friendly CLI for Confluence Cloud")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

pub async fn run() -> i32 {
    let cli = Cli::parse();
    let result = dispatch(cli.command).await;

    match result {
        Ok(output) => match print_success(&output) {
            Ok(()) => 0,
            Err(error) => {
                let _ = print_json(&error_json(output.command, &error));
                1
            }
        },
        Err(failure) => {
            let _ = print_json(&error_json(failure.command, &failure.error));
            1
        }
    }
}

fn print_success(output: &crate::commands::CommandOutput) -> Result<(), crate::error::AppError> {
    if let Some(text) = &output.text {
        return print_text(text);
    }

    print_json(&success_json(
        output.command,
        output.dry_run,
        output.data.clone(),
    ))
}
