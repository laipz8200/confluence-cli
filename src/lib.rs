pub mod cli;

pub async fn run() -> i32 {
    cli::run().await
}
