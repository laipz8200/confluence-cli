# confluence-cli

`confluence-cli` is an Agent-friendly CLI for Confluence Cloud. It exposes a small command surface with stable JSON output and safe dry-run writes.

## Install From Source

```bash
cargo install --path .
```

For local development:

```bash
cargo build --release
```

## Configure

Run:

```bash
confluence-cli config init
```

The default config path is:

```text
~/.config/confluence-cli/config.toml
```

Override it with:

```bash
CONFLUENCE_CLI_CONFIG=/path/to/config.toml confluence-cli space list
```

The API token is stored in plaintext. The CLI sets `0600` permissions on Unix platforms. Use a dedicated Atlassian API token and keep the config file out of source control.

## Commands

```bash
confluence-cli space list
confluence-cli search --query "deploy"
confluence-cli search --cql 'space = ENG and text ~ "deploy"'
confluence-cli page get --page-id 123456
confluence-cli page create --space-key ENG --title "New Page" --body-file page.md
confluence-cli page create --space-key ENG --title "New Page" --body-file page.md --execute
confluence-cli page update --page-id 123456 --title "Updated Page" --body-file page.md
confluence-cli page update --page-id 123456 --title "Updated Page" --body-file page.md --execute
```

All commands print JSON. Write commands are dry-run by default. A real create or update only happens when `--execute` is present.

## Agent Safety

Agents should run write commands without `--execute` first, inspect the returned JSON, and ask the user before executing the write. Updates require `--page-id`; the CLI does not update pages by title.

## Skills Package

The repository includes a generic Skills package at:

```text
skills/confluence-cli
```

Install it with the Skills installer used by your Agent environment, for example:

```bash
npx skills install ./skills/confluence-cli
```
