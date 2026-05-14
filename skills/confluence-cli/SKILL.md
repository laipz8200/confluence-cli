---
name: confluence-cli
description: Use when an Agent needs Confluence Cloud space listing, text or CQL search, page reads, safe page creation or updates, Markdown page bodies, or Confluence storage XML through the confluence-cli binary.
---

# Confluence CLI Skill

Use `confluence-cli` for Confluence Cloud work. Treat the CLI as the only supported interface. Do not call Confluence REST APIs directly from this skill.

## User Interaction Format

Prefer human-readable Markdown for user-facing drafts, summaries, approval prompts, and page body files.

Use raw Confluence storage XML only when it is necessary to preserve Confluence-specific structures that Markdown cannot represent, such as layouts or macros, or when the user explicitly provides or requests storage XML. When XML is necessary, explain the reason in Markdown and keep the XML in fenced code blocks or separate `.storage.xml` files instead of making XML the primary conversational format.

Markdown body files may use headings, paragraphs, emphasis, links, lists, tables, code blocks, block quotes, and strikethrough. Do not use images, attachments, raw HTML, or unsafe link schemes in Markdown body files. If the requested content requires one of those unsupported Markdown features, use storage XML only when it is necessary and explain why in Markdown.

## Prerequisite Check

Before using the CLI, run:

```bash
confluence-cli --version
```

`confluence-cli --version` prints normal CLI version text, not JSON.

If the command is missing, tell the user to install the latest release binary:

```bash
curl -fsSL https://raw.githubusercontent.com/laipz8200/confluence-cli/main/install.sh | sh
```

If they need a local source build, tell them to run this from the `confluence-cli` repository root or another source checkout:

```bash
cargo install --path .
```

If configuration is missing, ask the user to run:

```bash
confluence-cli config init
```

## Read Commands

List spaces:

```bash
confluence-cli space list
```

Search with simple text:

```bash
confluence-cli search --query "deploy"
```

Search with raw CQL:

```bash
confluence-cli search --cql 'space = ENG and text ~ "deploy"'
```

Read a page:

```bash
confluence-cli page get --page-id 123456
```

## Write Commands

Write commands are dry-run by default. Always run the dry-run first. Dry-run prevents create or update writes, but it may still read Confluence metadata: create resolves the space key, and update reads the current page version.

Create dry-run:

```bash
confluence-cli page create --space-key ENG --title "New Page" --body-file page.md
```

Create child page dry-run:

```bash
confluence-cli page create --space-key ENG --title "New Page" --body-file page.md --parent-id 123456
```

Create dry-run with raw Confluence storage XML only when Markdown is insufficient:

```bash
confluence-cli page create --space-key ENG --title "New Page" --body-file page.storage.xml
```

Update dry-run:

```bash
confluence-cli page update --page-id 123456 --title "Updated Page" --body-file page.md
```

Update dry-run with raw Confluence storage XML only when Markdown is insufficient:

```bash
confluence-cli page update --page-id 123456 --title "Updated Page" --body-file page.storage.xml
```

Only add `--execute` after you have run the dry-run, inspected the dry-run JSON, shown or summarized that dry-run result to the user, and the user explicitly approves that execution. After approval, rerun the same reviewed dry-run command with `--execute` appended. Do not use standalone execute examples as the starting point.

Body representation is inferred from the body file name. For new or edited prose, prefer Markdown files. Files ending in `.storage`, `.storage.xml`, or `.xml` are sent as raw Confluence storage XML. Other files are treated as Markdown. Use `--body-representation storage` or `--body-representation markdown` only when you need to override inference for a non-standard file name.

## Output Rules

Successful operational subcommands print JSON, and app-level validation or API errors print JSON envelopes. `--help`, `--version`, and clap argument parse errors may print normal CLI text. For JSON responses, check:

- `ok`: command success flag
- `command`: stable command name
- `dry_run`: true for dry-run write responses
- `data`: command result
- `error.code`: stable failure code when `ok` is false

## Safety Rules

- Do not print API tokens.
- Do not add `--execute` unless the dry-run has been run, its JSON has been inspected and shown or summarized to the user, and the user explicitly approves that execution.
- After approval, append `--execute` to the exact reviewed dry-run command instead of inventing a new command.
- Do not update pages by title.
- Do not create direct Confluence REST calls to bypass the CLI.
- Use storage mode only when Markdown cannot represent the required Confluence structure or the user explicitly provides or requests storage XML.
- In storage mode, treat the body file as authoritative Confluence storage XML and rely on Confluence's API validation.
- If `ok` is false, report `error.code` and `error.message` to the user.
