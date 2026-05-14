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
confluence-cli page create --space-key ENG --title "New Page" --body-file page.storage.xml
confluence-cli page update --page-id 123456 --title "Updated Page" --body-file page.md
confluence-cli page update --page-id 123456 --title "Updated Page" --body-file page.md --execute
confluence-cli page update --page-id 123456 --title "Updated Page" --body-file page.storage.xml
```

Successful operational subcommands print JSON, and app-level validation or API errors print JSON envelopes. `--help`, `--version`, and clap argument parse errors may print normal CLI text. Write commands are dry-run by default. A real create or update only happens when `--execute` is present.

Write commands infer the body representation from the body file name. Files ending in `.storage`, `.storage.xml`, or `.xml` are sent as Confluence storage XML unchanged, including layouts and macros such as `<ac:structured-macro>`. Other files are read as Markdown and converted to Confluence storage format. Use `--body-representation storage` or `--body-representation markdown` to override inference for non-standard file names.

## Agent Safety

Agents should run write commands without `--execute` first, inspect the returned dry-run JSON, show or summarize it to the user, and ask for explicit approval before executing the write. Updates require `--page-id`; the CLI does not update pages by title. In storage mode, the CLI sends the provided storage XML to Confluence unchanged and relies on Confluence to validate it.

## Skills Package

The repository includes a generic Skills package at:

```text
skills/confluence-cli
```

Install it with the Skills installer used by your Agent environment, for example:

```bash
npx skills install ./skills/confluence-cli
```

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
