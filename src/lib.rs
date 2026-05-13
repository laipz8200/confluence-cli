pub mod cli;
pub mod error;
pub mod output;

pub async fn run() -> i32 {
    cli::run().await
}
