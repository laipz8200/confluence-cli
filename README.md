<p align="center">
  <img src="assets/logo.svg" alt="confluence-cli logo" width="120" />
</p>

<h1 align="center">confluence-cli</h1>

<p align="center">
  <strong>Agent-friendly Confluence Cloud workflows from your terminal.</strong>
</p>

<p align="center">
  Stable JSON output, safe dry-run writes, Markdown-to-storage conversion, and a companion Skills package for automation-heavy teams.
</p>

<p align="center">
  <a href="LICENSE"><img alt="License: Apache-2.0" src="https://img.shields.io/badge/license-Apache--2.0-2f7d68?style=flat-square"></a>
  <img alt="Rust 2021" src="https://img.shields.io/badge/Rust-2021-f46623?style=flat-square">
  <img alt="Dry-run writes by default" src="https://img.shields.io/badge/writes-dry--run%20by%20default-4b5563?style=flat-square">
  <img alt="Agent friendly" src="https://img.shields.io/badge/agent-friendly-2563eb?style=flat-square">
</p>

<p align="center">
  English | <a href="README.zh-CN.md">简体中文</a>
</p>

## What It Does

`confluence-cli` is a small, focused CLI for Confluence Cloud. It is designed for humans and Agents that need predictable Confluence reads and safer page writes without wrapping the entire REST API.

- Stable JSON envelopes for operational commands.
- Search, page reads, space listing, page creation, and page updates.
- Write commands that dry-run by default and only write with `--execute`.
- Markdown input converted to Confluence storage format.
- Raw Confluence storage XML support when layouts or macros must be preserved.
- A companion Skills package that teaches Agents how to use the CLI safely.

## Install

Install the latest release binary:

```bash
curl -fsSL https://raw.githubusercontent.com/laipz8200/confluence-cli/main/install.sh | sh
```

The installer supports Linux x86_64, Linux arm64, macOS x86_64, and macOS arm64
release artifacts. It installs to `~/.local/bin` by default. Override the
install directory or version with environment variables:

```bash
curl -fsSL https://raw.githubusercontent.com/laipz8200/confluence-cli/main/install.sh | CONFLUENCE_CLI_INSTALL_DIR=/usr/local/bin sh
curl -fsSL https://raw.githubusercontent.com/laipz8200/confluence-cli/main/install.sh | CONFLUENCE_CLI_VERSION=0.1.0 sh
```

Install from a source checkout when you need a local build:

```bash
cargo install --path .
```

For local development builds:

```bash
cargo build --release
```

## Agent Skills Package

If you are using an Agent, `confluence-cli config init` asks whether to install the companion Skills package after saving your config. Press Enter to install it, or enter `n` to skip.

To install or reinstall it manually:

```bash
npx skills add laipz8200/confluence-cli --skill confluence-cli
```

The skill instructs Agents to use dry-runs first, summarize planned writes, and ask for explicit approval before adding `--execute`.

## Quick Start

Create your config:

```bash
confluence-cli config init
```

The setup verifies the Confluence API credentials, lists accessible spaces by
name, and lets you choose an optional default space. If no spaces are
accessible, the config is still saved without a default space.

List spaces:

```bash
confluence-cli space list
```

Search by text:

```bash
confluence-cli search --query "deploy"
```

Read a page:

```bash
confluence-cli page get --page-id 123456
```

Draft a page write safely:

```bash
confluence-cli page create --space-key ENG --title "Release Notes" --body-file release-notes.md
```

The create command above returns dry-run JSON and does not write to Confluence. Add `--execute` only after reviewing the dry-run output.

## Configuration

The default config path is:

```text
~/.config/confluence-cli/config.toml
```

Override it per command with:

```bash
CONFLUENCE_CLI_CONFIG=/path/to/config.toml confluence-cli space list
```

The API token is stored in plaintext. On Unix platforms, `confluence-cli` writes the config file with `0600` permissions. Use a dedicated Atlassian API token and keep the config file out of source control.

`default_space` is optional. When a config omits it, commands keep JSON output
on stdout and print a warning to stderr when they load the config.

## Command Map

| Command | Use it for | Notes |
| --- | --- | --- |
| `confluence-cli config init` | Create the local config file | Interactive setup |
| `confluence-cli space list` | List accessible spaces | JSON output |
| `confluence-cli search --query "deploy"` | Search by text | Builds a conservative CQL query |
| `confluence-cli search --cql 'space = ENG and text ~ "deploy"'` | Run explicit CQL | For advanced searches |
| `confluence-cli page get --page-id 123456` | Read page metadata and body | Uses page IDs, not titles |
| `confluence-cli page create --space-key ENG --title "New Page" --body-file page.md` | Preview a page create | Dry-run by default |
| `confluence-cli page create --space-key ENG --title "New Page" --body-file page.md --parent-id 123456` | Preview a child page create | Dry-run by default |
| `confluence-cli page update --page-id 123456 --title "Updated Page" --body-file page.md` | Preview a page update | Dry-run by default |

## Safe Writes

Write commands are intentionally two-step:

1. Run the create or update command without `--execute`.
2. Inspect the returned dry-run JSON.
3. Re-run the same command with `--execute` only after approval.

```bash
confluence-cli page update --page-id 123456 --title "Updated Page" --body-file page.md
confluence-cli page update --page-id 123456 --title "Updated Page" --body-file page.md --execute
```

Updates require `--page-id`; the CLI does not update pages by title. During update dry-runs, the CLI reads the current page version so the eventual write can use the next Confluence version number.

## Body Formats

By default, body format is inferred from the file name:

| File name | Representation |
| --- | --- |
| `page.md` | Markdown converted to Confluence storage HTML |
| `page.storage` | Raw Confluence storage XML |
| `page.storage.xml` | Raw Confluence storage XML |
| `page.xml` | Raw Confluence storage XML |

Use `--body-representation storage` or `--body-representation markdown` to override inference for non-standard file names.

Storage mode sends the file contents unchanged. Use it when Markdown cannot represent the page, such as Confluence layouts or macros like `<ac:structured-macro>`.

## JSON Contract

Successful operational commands return a stable envelope:

```json
{
  "ok": true,
  "command": "page.update",
  "dry_run": true,
  "data": {}
}
```

Application errors use the same shape with a stable error code:

```json
{
  "ok": false,
  "command": "page.update",
  "error": {
    "code": "confluence_version_conflict",
    "message": "Page was updated by someone else. Fetch the latest version and retry.",
    "retryable": true,
    "details": {}
  }
}
```

`--help`, `--version`, and clap argument parse errors may print normal CLI text.

## Development

Run the test suite:

```bash
cargo test --locked
```

Run Clippy before opening a pull request:

```bash
cargo clippy --locked --all-targets --all-features -- -D warnings
```

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
