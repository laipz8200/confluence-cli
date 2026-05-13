#[tokio::main]
async fn main() {
    std::process::exit(confluence_cli::run().await);
}
