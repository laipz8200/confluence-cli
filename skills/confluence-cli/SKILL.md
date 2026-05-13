---
name: confluence-cli
description: Use when an Agent needs to search Confluence, read pages, create pages, or update pages through the confluence-cli binary.
---

# Confluence CLI Skill

Use `confluence-cli` for Confluence Cloud work. Treat the CLI as the only supported interface. Do not call Confluence REST APIs directly from this skill.

## Prerequisite Check

Before using the CLI, run:

```bash
confluence-cli --version
```

If the command is missing, tell the user to install it from the repository:

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

Write commands are dry-run by default. Always run the dry-run first.

Create dry-run:

```bash
confluence-cli page create --space-key ENG --title "New Page" --body-file page.md
```

Update dry-run:

```bash
confluence-cli page update --page-id 123456 --title "Updated Page" --body-file page.md
```

Only add `--execute` after the user explicitly approves the write.

Create execute:

```bash
confluence-cli page create --space-key ENG --title "New Page" --body-file page.md --execute
```

Update execute:

```bash
confluence-cli page update --page-id 123456 --title "Updated Page" --body-file page.md --execute
```

## Output Rules

Every command prints JSON. Check:

- `ok`: command success flag
- `command`: stable command name
- `dry_run`: true for dry-run write responses
- `data`: command result
- `error.code`: stable failure code when `ok` is false

## Safety Rules

- Do not print API tokens.
- Do not add `--execute` unless the user explicitly asks for the write to happen.
- Do not update pages by title.
- Do not create direct Confluence REST calls to bypass the CLI.
- If `ok` is false, report `error.code` and `error.message` to the user.
