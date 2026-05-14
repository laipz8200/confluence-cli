# Command Module Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the command layer so executable commands live in focused files while the public CLI behavior and JSON contract stay unchanged.

**Architecture:** Keep `clap` static derive parsing, but move top-level command registration and dispatch into `src/commands/mod.rs`. Add `CommandContext` for shared config/client setup, introduce command output/failure types, and split `page get`, `page create`, and `page update` into separate command files. `src/cli.rs` becomes a thin parse/output wrapper.

**Tech Stack:** Rust 2021, clap derive, tokio, reqwest, serde_json, assert_cmd, wiremock, tempfile.

---

## File Structure

Create these files:

- `src/command_context.rs`: shared command execution context that loads config and builds `ConfluenceClient`.
- `src/commands/config_init.rs`: `config init` args, prompts, and execution.
- `src/commands/page_body.rs`: shared page body representation and body file conversion.
- `src/commands/page_create.rs`: `page create` args and execution.
- `src/commands/page_get.rs`: `page get` args and execution.
- `src/commands/page_update.rs`: `page update` args and execution.
- `src/commands/space_list.rs`: `space list` args and execution.

Modify these files:

- `src/lib.rs`: export `command_context`.
- `src/cli.rs`: keep only parse, dispatch call, JSON output, and exit code logic.
- `src/commands/mod.rs`: own command enums, command result types, module registration, and dispatch glue.
- `src/commands/search.rs`: add `Args` and accept `CommandContext`.
- `tests/config_contract.rs`: add a narrow `CommandContext` loading test.

Delete these files after their contents are migrated:

- `src/commands/config.rs`
- `src/commands/page.rs`
- `src/commands/space.rs`

---

### Task 1: Add Command Context

**Files:**
- Create: `src/command_context.rs`
- Modify: `src/lib.rs`
- Modify: `tests/config_contract.rs`

- [ ] **Step 1: Write the failing context-loading test**

In `tests/config_contract.rs`, add this import near the existing `confluence_cli` imports:

```rust
use confluence_cli::command_context::CommandContext;
```

Add this test after `config_allows_loopback_http_for_mock_servers`:

```rust
#[test]
fn command_context_loads_client_from_config_env_var() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let config = Config {
        site_url: "http://127.0.0.1:12345/wiki".to_string(),
        email: "user@example.com".to_string(),
        api_token: "token-value".to_string(),
        default_space: "ENG".to_string(),
    };
    save_config(&path, &config).unwrap();

    let _guard = EnvVarGuard::set("CONFLUENCE_CLI_CONFIG", &path);
    let context = CommandContext::load().unwrap();

    let _client = context.client();
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test --locked --test config_contract command_context_loads_client_from_config_env_var
```

Expected: FAIL with an unresolved import for `confluence_cli::command_context`.

- [ ] **Step 3: Implement `CommandContext`**

Create `src/command_context.rs`:

```rust
use crate::client::ConfluenceClient;
use crate::config::load_default_config;
use crate::error::AppError;

pub struct CommandContext {
    client: ConfluenceClient,
}

impl CommandContext {
    pub fn load() -> Result<Self, AppError> {
        let config = load_default_config()?;
        let client = ConfluenceClient::new(config)?;

        Ok(Self { client })
    }

    pub fn client(&self) -> &ConfluenceClient {
        &self.client
    }
}
```

Add `command_context` to `src/lib.rs`:

```rust
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
```

- [ ] **Step 4: Run the focused test to verify it passes**

Run:

```bash
cargo test --locked --test config_contract command_context_loads_client_from_config_env_var
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/command_context.rs src/lib.rs tests/config_contract.rs
git commit -m "refactor(commands): add command context"
```

---

### Task 2: Move Command Registration And Dispatch Out Of `cli.rs`

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/commands/mod.rs`

- [ ] **Step 1: Run the existing CLI smoke test as a behavior guard**

Run:

```bash
cargo test --locked --test cli_smoke
```

Expected: PASS for `version_flag_prints_package_version` and `help_mentions_agent_first_commands`.

- [ ] **Step 2: Replace `src/commands/mod.rs` with command registration and dispatch**

Replace `src/commands/mod.rs` with:

```rust
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
            } => page::update(
                &page_id,
                &title,
                &body_file,
                body_representation,
                execute,
            )
            .await
            .map(|(dry_run, data)| CommandOutput::new("page.update", dry_run, data))
            .map_err(|error| CommandFailure::new("page.update", error)),
        },
    }
}
```

- [ ] **Step 3: Replace `src/cli.rs` with a thin parser/output wrapper**

Replace `src/cli.rs` with:

```rust
use crate::commands::{dispatch, Commands};
use crate::output::{error_json, print_json, success_json};
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
        Ok(output) => match print_json(&success_json(
            output.command,
            output.dry_run,
            output.data,
        )) {
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
```

- [ ] **Step 4: Run the CLI smoke test to verify behavior is unchanged**

Run:

```bash
cargo test --locked --test cli_smoke
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/commands/mod.rs
git commit -m "refactor(cli): move command dispatch"
```

---

### Task 3: Split Config, Space, Search, And Page Get Commands

**Files:**
- Create: `src/commands/config_init.rs`
- Create: `src/commands/page_get.rs`
- Create: `src/commands/space_list.rs`
- Modify: `src/commands/mod.rs`
- Modify: `src/commands/search.rs`
- Delete: `src/commands/config.rs`
- Delete: `src/commands/space.rs`

- [ ] **Step 1: Run read-command tests as behavior guards**

Run:

```bash
cargo test --locked --test config_contract --test read_commands
```

Expected: PASS.

- [ ] **Step 2: Create `src/commands/config_init.rs`**

Create `src/commands/config_init.rs`:

```rust
use crate::commands::{CommandOutput, CommandResult};
use crate::config::{config_path, save_config, Config};
use crate::error::AppError;
use clap::Args;
use serde_json::json;
use std::io::{self, Write};

pub const COMMAND: &str = "config.init";

#[derive(Debug, Args)]
pub struct ConfigInitArgs {}

pub fn run(_args: ConfigInitArgs) -> CommandResult {
    let site_url = prompt("Confluence site URL")?;
    let email = prompt("Email")?;
    let api_token = read_api_token()?;
    let default_space = prompt("Default space key")?;

    let config = Config {
        site_url,
        email,
        api_token,
        default_space,
    };
    let path = config_path()?;
    save_config(&path, &config)?;

    Ok(CommandOutput::new(
        COMMAND,
        false,
        json!({
            "path": path,
            "site_url": config.site_url.trim_end_matches('/'),
            "email": config.email,
            "default_space": config.default_space
        }),
    ))
}

fn prompt(label: &str) -> Result<String, AppError> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stderr = io::stderr();
    let mut prompt_output = stderr.lock();
    prompt_with_io(label, &mut input, &mut prompt_output)
}

fn read_api_token() -> Result<String, AppError> {
    match rpassword::prompt_password("API token: ") {
        Ok(api_token) => Ok(api_token.trim().to_string()),
        Err(source) if is_tty_unavailable(&source) => prompt("API token"),
        Err(source) => Err(AppError::new(
            crate::error::ErrorCode::ConfigInvalid,
            format!("Failed to read API token: {source}"),
        )),
    }
}

fn is_tty_unavailable(source: &io::Error) -> bool {
    matches!(
        source.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::Unsupported
    ) || source.raw_os_error() == Some(6)
}

fn prompt_with_io(
    label: &str,
    input: &mut impl io::BufRead,
    prompt_output: &mut impl Write,
) -> Result<String, AppError> {
    write!(prompt_output, "{label}: ").map_err(|source| {
        AppError::new(
            crate::error::ErrorCode::ConfigInvalid,
            format!("Failed to write prompt: {source}"),
        )
    })?;
    prompt_output.flush().map_err(|source| {
        AppError::new(
            crate::error::ErrorCode::ConfigInvalid,
            format!("Failed to flush prompt: {source}"),
        )
    })?;
    let mut value = String::new();
    input.read_line(&mut value).map_err(|source| {
        AppError::new(
            crate::error::ErrorCode::ConfigInvalid,
            format!("Failed to read {label}: {source}"),
        )
    })?;
    Ok(value.trim().to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn prompt_helper_writes_prompt_to_injected_non_stdout_stream() {
        let mut input = " ENG \n".as_bytes();
        let mut prompt_output = Vec::new();

        let value =
            super::prompt_with_io("Default space key", &mut input, &mut prompt_output).unwrap();

        assert_eq!(value, "ENG");
        assert_eq!(
            String::from_utf8(prompt_output).unwrap(),
            "Default space key: "
        );
    }
}
```

- [ ] **Step 3: Create `src/commands/space_list.rs`**

Create `src/commands/space_list.rs`:

```rust
use crate::command_context::CommandContext;
use crate::commands::{CommandOutput, CommandResult};
use clap::Args;
use serde_json::json;

pub const COMMAND: &str = "space.list";

#[derive(Debug, Args)]
pub struct SpaceListArgs {}

pub async fn run(_args: SpaceListArgs, ctx: CommandContext) -> CommandResult {
    let spaces = ctx.client().list_spaces().await?;

    Ok(CommandOutput::new(
        COMMAND,
        false,
        json!({ "spaces": spaces }),
    ))
}
```

- [ ] **Step 4: Replace `src/commands/search.rs` with args plus context**

Replace `src/commands/search.rs` with:

```rust
use crate::command_context::CommandContext;
use crate::commands::{CommandOutput, CommandResult};
use crate::error::{AppError, ErrorCode};
use clap::Args;
use serde_json::json;

pub const COMMAND: &str = "search";

#[derive(Debug, Args)]
pub struct SearchArgs {
    #[arg(long)]
    query: Option<String>,
    #[arg(long)]
    cql: Option<String>,
}

pub async fn run(args: SearchArgs, ctx: CommandContext) -> CommandResult {
    let cql = match (args.query, args.cql) {
        (Some(query), None) => format!("text ~ \"{}\"", escape_cql_text(&query)),
        (None, Some(cql)) => cql,
        _ => {
            return Err(AppError::new(
                ErrorCode::ConfluenceValidationFailed,
                "Provide exactly one of --query or --cql.",
            ));
        }
    };

    let result = ctx.client().search(&cql).await?;

    Ok(CommandOutput::new(
        COMMAND,
        false,
        json!({ "cql": cql, "result": result }),
    ))
}

fn escape_cql_text(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    #[test]
    fn escape_cql_text_escapes_backslashes_and_quotes() {
        assert_eq!(
            super::escape_cql_text(r#"deploy \ "guide""#),
            r#"deploy \\ \"guide\""#
        );
    }
}
```

- [ ] **Step 5: Create `src/commands/page_get.rs`**

Create `src/commands/page_get.rs`:

```rust
use crate::command_context::CommandContext;
use crate::commands::{CommandOutput, CommandResult};
use crate::error::{AppError, ErrorCode};
use clap::Args;

pub const COMMAND: &str = "page.get";

#[derive(Debug, Args)]
pub struct PageGetArgs {
    #[arg(long)]
    page_id: String,
}

pub async fn run(args: PageGetArgs, ctx: CommandContext) -> CommandResult {
    let page = ctx.client().get_page(&args.page_id).await?;
    let data = serde_json::to_value(page).map_err(|source| {
        AppError::new(
            ErrorCode::InternalError,
            format!("Failed to serialize page JSON: {source}"),
        )
    })?;

    Ok(CommandOutput::new(COMMAND, false, data))
}
```

- [ ] **Step 6: Replace `src/commands/mod.rs` to use the split read commands**

Replace `src/commands/mod.rs` with:

```rust
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
            let ctx = load_context(search::COMMAND)?;
            search::run(args, ctx)
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
```

- [ ] **Step 7: Delete migrated command files**

Delete:

```bash
rm src/commands/config.rs src/commands/space.rs
```

- [ ] **Step 8: Run focused behavior tests**

Run:

```bash
cargo test --locked --test config_contract --test read_commands
```

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add src/commands/mod.rs src/commands/config_init.rs src/commands/page_get.rs src/commands/search.rs src/commands/space_list.rs
git rm src/commands/config.rs src/commands/space.rs
git commit -m "refactor(commands): split read commands"
```

---

### Task 4: Split Page Create And Page Update

**Files:**
- Create: `src/commands/page_body.rs`
- Create: `src/commands/page_create.rs`
- Create: `src/commands/page_update.rs`
- Modify: `src/commands/mod.rs`
- Delete: `src/commands/page.rs`

- [ ] **Step 1: Run write-command tests as behavior guards**

Run:

```bash
cargo test --locked --test write_commands --test content_contract
```

Expected: PASS.

- [ ] **Step 2: Create `src/commands/page_body.rs`**

Create `src/commands/page_body.rs`:

```rust
use crate::content::{markdown_to_storage, storage_to_storage, ConvertedContent};
use crate::error::{AppError, ErrorCode};
use clap::ValueEnum;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum BodyRepresentation {
    Markdown,
    Storage,
}

pub fn read_body_file(
    body_file: &Path,
    body_representation: Option<BodyRepresentation>,
) -> Result<ConvertedContent, AppError> {
    let body = std::fs::read_to_string(body_file).map_err(|source| {
        AppError::new(
            ErrorCode::MarkdownConversionFailed,
            format!("Failed to read body file: {source}"),
        )
    })?;

    match body_representation.unwrap_or_else(|| infer_body_representation(body_file)) {
        BodyRepresentation::Markdown => markdown_to_storage(&body),
        BodyRepresentation::Storage => storage_to_storage(&body),
    }
}

fn infer_body_representation(body_file: &Path) -> BodyRepresentation {
    let Some(file_name) = body_file.file_name().and_then(|name| name.to_str()) else {
        return BodyRepresentation::Markdown;
    };
    let file_name = file_name.to_ascii_lowercase();

    if file_name.ends_with(".storage.xml")
        || file_name.ends_with(".storage")
        || file_name.ends_with(".xml")
    {
        BodyRepresentation::Storage
    } else {
        BodyRepresentation::Markdown
    }
}
```

- [ ] **Step 3: Create `src/commands/page_create.rs`**

Create `src/commands/page_create.rs`:

```rust
use crate::client::CreatePageRequest;
use crate::command_context::CommandContext;
use crate::commands::page_body::{read_body_file, BodyRepresentation};
use crate::commands::{CommandOutput, CommandResult};
use crate::dry_run::{create_dry_run, WriteTarget};
use clap::Args;
use std::path::PathBuf;

pub const COMMAND: &str = "page.create";

#[derive(Debug, Args)]
pub struct PageCreateArgs {
    #[arg(long)]
    space_key: String,
    #[arg(long)]
    title: String,
    #[arg(long)]
    body_file: PathBuf,
    #[arg(long, value_enum)]
    body_representation: Option<BodyRepresentation>,
    #[arg(long)]
    parent_id: Option<String>,
    #[arg(long)]
    execute: bool,
}

pub async fn run(args: PageCreateArgs, ctx: CommandContext) -> CommandResult {
    let converted = read_body_file(&args.body_file, args.body_representation)?;
    let space_id = ctx.client().resolve_space_id(&args.space_key).await?;

    if !args.execute {
        return Ok(CommandOutput::new(
            COMMAND,
            true,
            create_dry_run(
                "POST",
                "/api/v2/pages",
                WriteTarget::Create {
                    space_key: args.space_key,
                    space_id,
                    parent_id: args.parent_id,
                },
                &args.title,
                &converted,
            ),
        ));
    }

    let response = ctx
        .client()
        .create_page(CreatePageRequest {
            space_id,
            title: args.title,
            parent_id: args.parent_id,
            storage_html: converted.storage_html,
        })
        .await?;

    Ok(CommandOutput::new(COMMAND, false, response))
}
```

- [ ] **Step 4: Create `src/commands/page_update.rs`**

Create `src/commands/page_update.rs`:

```rust
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
```

- [ ] **Step 5: Replace `src/commands/mod.rs` to use split page write commands**

Replace `src/commands/mod.rs` with:

```rust
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
            let ctx = load_context(search::COMMAND)?;
            search::run(args, ctx)
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
```

- [ ] **Step 6: Delete the old combined page command file**

Run:

```bash
rm src/commands/page.rs
```

- [ ] **Step 7: Run focused write-command and content tests**

Run:

```bash
cargo test --locked --test write_commands --test content_contract
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src/commands/mod.rs src/commands/page_body.rs src/commands/page_create.rs src/commands/page_get.rs src/commands/page_update.rs
git rm src/commands/page.rs
git commit -m "refactor(commands): split page commands"
```

---

### Task 5: Final Verification And Cleanup

**Files:**
- Verify: `src/cli.rs`
- Verify: `src/commands/*`
- Verify: `tests/*`

- [ ] **Step 1: Check that old combined command files are gone**

Run:

```bash
test ! -e src/commands/config.rs
test ! -e src/commands/space.rs
test ! -e src/commands/page.rs
```

Expected: all three commands exit 0.

- [ ] **Step 2: Check that `cli.rs` no longer owns command-specific definitions**

Run:

```bash
rg -n "ConfigCommand|SpaceCommand|PageCommand|BodyRepresentation|page_id|space_key|body_file" src/cli.rs
```

Expected: no matches and exit code 1.

- [ ] **Step 3: Check that command files no longer build clients directly**

Run:

```bash
rg -n "load_default_config\\(|ConfluenceClient::new" src/commands
```

Expected: no matches and exit code 1.

- [ ] **Step 4: Run the full test suite**

Run:

```bash
cargo test --locked
```

Expected: PASS.

- [ ] **Step 5: Run Clippy**

Run:

```bash
cargo clippy --locked --all-targets --all-features -- -D warnings
```

Expected: PASS.

- [ ] **Step 6: Review the final diff**

Run:

```bash
git status --short
git diff --stat HEAD~4..HEAD
```

Expected: `git status --short` prints nothing. The diff stat shows command-layer refactor files and the `command_context` addition.
