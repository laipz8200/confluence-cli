# Command Module Refactor Design

Date: 2026-05-14

## Purpose

Refactor the command layer so each command is easier to understand, change, and
test independently. The refactor should reduce the current coupling in
`src/cli.rs` and `src/commands/page.rs` without changing the public CLI surface
or JSON output contract.

## Goals

- Keep the existing command behavior, flags, stable command names, JSON
  envelopes, and dry-run safety unchanged.
- Move command-specific argument structs and execution logic into focused
  command files.
- Keep `src/cli.rs` as a thin parser and output envelope entry point.
- Introduce a shared command context for common config loading and client
  construction.
- Make future command additions require a new command file plus a small,
  centralized registration change.
- Preserve the existing test suite as the main behavior guard.

## Non-Goals

- Do not introduce a dynamic plugin system.
- Do not change the documented command names or command-line flags.
- Do not rewrite the HTTP client or Confluence API DTOs.
- Do not split the crate into multiple crates.
- Do not broaden the Confluence feature set.

## Current Problems

`src/cli.rs` currently owns top-level command definitions, nested command
definitions, command-specific flags, dispatch, and JSON envelope wiring. This
makes every command addition touch a large central file and mixes CLI plumbing
with business-level command details.

`src/commands/page.rs` currently contains `page get`, `page create`, and
`page update`. These operations share the `page` namespace, but they have
different dependencies and safety behavior. Combining them makes the file grow
around unrelated responsibilities.

Network command modules repeat the same setup pattern:

```text
load_default_config()
  -> ConfluenceClient::new(config)
```

That common setup belongs in one shared command execution module rather than in
each command file.

## Proposed Structure

Use a conservative static structure compatible with `clap` derive:

```text
src/
  cli.rs
  command_context.rs
  commands/
    mod.rs
    config_init.rs
    page_create.rs
    page_get.rs
    page_update.rs
    search.rs
    space_list.rs
```

`src/cli.rs` remains responsible for:

- parsing the top-level CLI;
- delegating to `commands::dispatch`;
- printing success and error JSON envelopes;
- returning the process exit code.

`src/commands/mod.rs` becomes the centralized registration and dispatch layer.
It owns the top-level `Commands` enum and namespace enums such as `PageCommand`.
It should avoid command-specific business logic. Each branch should only call
the matching command module and attach the stable command name on failure when
needed.

Each command file owns its command-specific arguments and execution:

```rust
#[derive(Debug, clap::Args)]
pub struct Args {
    #[arg(long)]
    page_id: String,
}

pub async fn run(args: Args, ctx: CommandContext) -> CommandResult {
    // command-specific orchestration
}
```

This keeps flags near the command implementation while preserving static
compile-time command registration.

## Shared Command Context

Add `src/command_context.rs` with a narrow API:

```rust
pub struct CommandContext {
    client: ConfluenceClient,
}

impl CommandContext {
    pub fn load() -> Result<Self, AppError>;
    pub fn client(&self) -> &ConfluenceClient;
}
```

The context loads the default config and builds a `ConfluenceClient`. Commands
that need Confluence call `CommandContext::load()` through the dispatch layer.
`config init` does not use this context because it creates configuration rather
than consuming it.

The context should not become a service locator. It should only hold execution
state that is broadly shared by commands.

## Command Result Types

Introduce command-level result types to reduce tuple plumbing:

```rust
pub struct CommandOutput {
    pub command: &'static str,
    pub dry_run: bool,
    pub data: serde_json::Value,
}

pub type CommandResult = Result<CommandOutput, AppError>;
```

Command modules return `CommandOutput` with their stable command name, such as
`page.create`. `cli.rs` converts `CommandOutput` into the existing success
envelope. On errors, dispatch preserves the stable command name for the error
envelope.

This is an internal refactor only. The external success and failure JSON shapes
must remain unchanged.

## Command Boundaries

The command files should be split by executable command, not just by command
namespace:

- `config_init.rs`: interactive config initialization.
- `space_list.rs`: `space list`.
- `search.rs`: simplified query and raw CQL validation plus search execution.
- `page_get.rs`: read one page with storage body.
- `page_create.rs`: body file conversion, space resolution, dry-run, and create.
- `page_update.rs`: body file conversion, version read, dry-run, and update.

Shared page body handling should move out of `page_create.rs` and
`page_update.rs` only if duplication becomes meaningful. A small helper module
is acceptable, but the first implementation should prefer the smallest
extraction needed to keep both command files focused.

## Data Flow

For network commands:

```text
clap parse
  -> commands::dispatch
  -> CommandContext::load()
  -> command::run(args, ctx)
  -> CommandOutput
  -> cli.rs success JSON envelope
```

For `config init`:

```text
clap parse
  -> commands::dispatch
  -> config_init::run(args)
  -> CommandOutput
  -> cli.rs success JSON envelope
```

Errors follow the same path back to `cli.rs`, where they are printed with the
existing error JSON envelope.

## Error Handling

The refactor must preserve existing `AppError` codes and messages unless a test
exposes an accidental mismatch. Command dispatch must always know the stable
command name for the selected branch so failures continue to report values such
as `search`, `page.create`, and `page.update`.

No command should convert another command's error. Shared modules such as
`client`, `content`, `dry_run`, and `config` continue to own their current error
mapping responsibilities.

## Testing

The existing tests remain the primary acceptance criteria:

```bash
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
```

The refactor should add focused tests only where they protect new internal
boundaries, such as:

- `CommandOutput` conversion into the current JSON envelope, if that conversion
  moves out of `cli.rs`;
- `CommandContext::load()` failure behavior, if it receives non-trivial logic.

The test suite should not assert that a future command requires only one file,
because static `clap` registration intentionally requires a small centralized
registration change.

## Migration Plan

1. Add `command_context` and command result types without changing behavior.
2. Move command enums and dispatch glue out of `cli.rs` into `commands::mod`.
3. Split `config init`, `space list`, `page get`, `page create`, and
   `page update` into focused files.
4. Remove duplicated config/client setup from command files by using
   `CommandContext`.
5. Run the full test and Clippy checks.

Each step should keep the repository compiling before moving to the next step.

## Acceptance Criteria

- Existing CLI commands and flags behave the same as before.
- Existing success and error JSON envelopes are unchanged.
- Existing stable command names are unchanged.
- `src/cli.rs` no longer contains command-specific flag definitions or business
  dispatch details.
- No command file contains implementation logic for another executable command.
- Common config/client setup is centralized in `command_context`.
- `cargo test --locked` passes.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` passes.
