# Confluence CLI Design

Date: 2026-05-13

## Purpose

Create `confluence-cli`, a Rust command-line tool that exposes a small, safe, Agent-friendly interface over Confluence Cloud REST APIs.

The first release is not a complete REST API wrapper. It focuses on stable JSON output, predictable write safety, and a companion Skills package that Agents can install and use through the CLI contract.

## Goals

- Support Confluence Cloud using Atlassian account email and API token authentication.
- Provide a small set of high-value commands for Agent workflows.
- Default every command to machine-readable JSON output.
- Make write operations safe by default through dry-run behavior.
- Accept Markdown for page body input and convert it to Confluence storage HTML.
- Provide a generic Skills package installable through `npx skills`, compatible with Codex and other Skills-aware Agents.
- Prioritize source builds for the first release.

## Non-Goals

- Support Confluence Data Center or Server in the first release.
- Wrap every Confluence REST endpoint.
- Support attachments, comments, labels, permissions, macros, or page tree management in the first release.
- Store credentials in OS keychains in the first release.
- Support multiple profiles in the first release.
- Provide a low-level arbitrary REST `request` command in the first release.

## Command Surface

The binary name is `confluence-cli`.

Initial commands:

```text
confluence-cli config init
confluence-cli space list
confluence-cli search --query <text>
confluence-cli search --cql <cql>
confluence-cli page get --page-id <id>
confluence-cli page create --space-key <key> --title <title> --body-file <md> [--parent-id <id>] [--execute]
confluence-cli page update --page-id <id> --title <title> --body-file <md> [--execute]
```

Search supports both a simplified `--query` mode and a raw `--cql` mode. The simplified mode builds a conservative CQL query for common text search. Raw CQL is available for advanced Agent workflows.

Page updates must use `--page-id`. The first release does not update pages by title, space key, or parent title because those locators can be ambiguous.

Page creation accepts a user-facing `--space-key` argument. Before creating a page, the CLI resolves the space key to the Confluence Cloud space id required by the v2 page API. If no matching space is found, the command fails with `not_found`.

Page creation accepts an optional `--parent-id`. If omitted, the page is created in the target space without a parent according to Confluence Cloud behavior.

## Architecture

The Rust crate is split into small modules with clear ownership:

- `cli`: command definitions, argument parsing, and local argument validation.
- `config`: configuration path resolution, TOML read/write, `config init`, and file permission handling.
- `auth`: Atlassian Cloud Basic Auth header construction from email and API token.
- `client`: Confluence HTTP client, endpoint construction, pagination, status handling, and response deserialization.
- `commands`: command-level orchestration for spaces, search, and page operations.
- `content`: Markdown parsing and conversion to Confluence storage HTML.
- `output`: stable JSON success and error envelopes.
- `dry_run`: dry-run response construction for write commands.

The companion Skills package depends only on the `confluence-cli` binary and the documented JSON protocol. It must not depend on Rust internals.

## Configuration

The first release uses a single global profile. The default config path is:

```text
~/.config/confluence-cli/config.toml
```

Users can override the path with:

```text
CONFLUENCE_CLI_CONFIG=/path/to/config.toml
```

Config fields:

```toml
site_url = "https://example.atlassian.net/wiki"
email = "user@example.com"
api_token = "..."
default_space = "ENG"
```

`config init` interactively prompts for these values and writes the TOML file. The API token is stored in plaintext for the first release, and the CLI should set file permissions to `0600` where the platform supports it. Documentation and the Skills package must clearly state this security tradeoff.

## Authentication

Authentication targets Confluence Cloud only. The CLI builds an HTTP Basic Auth credential from:

```text
email:api_token
```

The resulting `Authorization` header is added by the HTTP client. Tokens must never be printed in command output, errors, debug logs, dry-run responses, or test snapshots.

## Data Flow

All commands follow the same high-level flow:

```text
CLI args
  -> config load
  -> auth/client build
  -> command validation
  -> optional markdown conversion
  -> dry-run JSON or HTTP request
  -> normalized JSON output
```

Read operations perform the HTTP request immediately and return normalized JSON.

Write operations first convert Markdown to Confluence storage HTML and construct the intended payload. Without `--execute`, the command returns a dry-run JSON response and must not call the create or update endpoint. With `--execute`, the command sends the write request.

`page update` first reads the current page version, then sends an update request with `version.number + 1`. This prevents blind overwrites and produces a clear version conflict error if Confluence rejects the update.

## JSON Output Protocol

Every command writes JSON to stdout by default.

Successful commands use this envelope:

```json
{
  "ok": true,
  "command": "page.update",
  "dry_run": true,
  "data": {}
}
```

Failed commands use this envelope and exit with a non-zero status:

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

The `command` value is stable and uses dot-separated names such as `config.init`, `space.list`, `search`, `page.get`, `page.create`, and `page.update`.

Dry-run responses for write commands include:

- `dry_run: true`
- HTTP method and endpoint path
- target space key or page id
- title
- parent page id when provided
- body format
- body summary with Markdown byte length, generated storage HTML byte length, and up to five detected headings
- sanitized payload preview with non-secret request fields, excluding the full generated storage HTML body

Dry-run responses must not include credentials.

## Error Model

Errors are normalized into stable codes so Agents do not need to parse human text.

Initial error categories:

- `config_not_found`
- `config_invalid`
- `auth_failed`
- `permission_denied`
- `not_found`
- `rate_limited`
- `confluence_validation_failed`
- `confluence_version_conflict`
- `network_error`
- `markdown_conversion_failed`
- `unsupported_markdown`
- `internal_error`

HTTP status mapping:

- `401` maps to `auth_failed`.
- `403` maps to `permission_denied`.
- `404` maps to `not_found`.
- `409` maps to `confluence_version_conflict` when returned from page update.
- `429` maps to `rate_limited`.
- Other 4xx responses map to `confluence_validation_failed`.
- 5xx and transport failures map to `network_error`.

The CLI may include a sanitized Confluence response summary in `error.details`, but it must avoid secrets and excessive HTML payloads.

## Markdown Support

The first release supports a pragmatic Markdown subset:

- headings
- paragraphs
- bold and italic text
- links
- ordered and unordered lists
- fenced code blocks
- inline code
- block quotes
- tables

The first release does not support:

- image upload or attachment creation
- Confluence macros
- complex Confluence layouts
- task lists
- permission changes

Unsupported Markdown should either degrade to plain HTML when safe or fail with `unsupported_markdown` when conversion would produce unclear Confluence output.

## Write Safety

Write commands are dry-run by default. A write request is only sent when the user passes `--execute`.

The Skills package must instruct Agents to:

- run create and update commands without `--execute` first;
- inspect the returned dry-run JSON;
- ask the user before executing real writes;
- add `--execute` only after explicit user approval;
- never update by title;
- never bypass the CLI with raw REST calls for unsupported write behavior.

## Skills Package

The repository includes a Skills package installable through `npx skills`. The package is generic and compatible with Codex as well as other Skills-aware Agents that support this distribution model.

The Skills package includes:

- usage triggers: searching Confluence, reading pages, creating pages, updating pages;
- prerequisite checks, including `confluence-cli --version`;
- configuration guidance using `confluence-cli config init`;
- read command examples;
- write dry-run examples;
- execute examples gated behind explicit user approval;
- JSON parsing guidance for `ok`, `dry_run`, `data`, and `error.code`;
- safety rules for credentials and writes.

The skill must treat `confluence-cli` as the only supported interface. It does not import Rust code, call private modules, or construct Confluence API requests directly.

## Repository Layout

Expected initial layout:

```text
Cargo.toml
README.md
examples/
  config.toml
src/
  main.rs
  cli.rs
  config.rs
  auth.rs
  client.rs
  output.rs
  content.rs
  dry_run.rs
  commands/
skills/
  confluence-cli/
    SKILL.md
    skill package manifest for npx skills installation
docs/
  superpowers/
    specs/
      2026-05-13-confluence-cli-design.md
```

## Installation Strategy

The first release prioritizes source builds:

```text
cargo install --path .
cargo build --release
```

README documentation explains how to install the CLI and how to install the companion Skills package separately.

Publishing to crates.io is deferred until the CLI contract and Skills package have been exercised in real Agent workflows.

## Testing Strategy

Unit tests cover:

- config path resolution;
- TOML read/write;
- config validation;
- Basic Auth construction without leaking token values;
- Markdown conversion;
- JSON success and error envelopes;
- HTTP status to error-code mapping.

HTTP tests use a mock server to verify:

- endpoint paths;
- HTTP methods;
- auth header presence;
- pagination behavior;
- create payloads;
- update version reads and version increment;
- write endpoints are not called during dry-run.

CLI integration tests use temporary config files and mock Confluence responses to verify:

- stdout is valid JSON;
- error paths return non-zero exit codes;
- dry-run commands do not write;
- `--execute` sends the expected request;
- `search --query` and `search --cql` behave distinctly.

## External References

- Confluence Cloud REST API v2 page endpoints: https://developer.atlassian.com/cloud/confluence/rest/v2/api-group-page/
- Confluence Cloud REST API v1 search endpoint: https://developer.atlassian.com/cloud/confluence/rest/v1/api-group-search/
- Atlassian account API token documentation: https://support.atlassian.com/atlassian-account/docs/manage-api-tokens-for-your-atlassian-account/
