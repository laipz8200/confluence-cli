<p align="center">
  <img src="assets/logo.svg" alt="confluence-cli logo" width="120" />
</p>

<h1 align="center">confluence-cli</h1>

<p align="center">
  <strong>面向 Agent 友好的终端 Confluence Cloud 工作流。</strong>
</p>

<p align="center">
  稳定的 JSON 输出、安全的 dry-run 写入、Markdown 到 storage 格式转换，以及面向自动化密集团队的配套 Skills 包。
</p>

<p align="center">
  <a href="LICENSE"><img alt="License: Apache-2.0" src="https://img.shields.io/badge/license-Apache--2.0-2f7d68?style=flat-square"></a>
  <img alt="Rust 2021" src="https://img.shields.io/badge/Rust-2021-f46623?style=flat-square">
  <img alt="Dry-run writes by default" src="https://img.shields.io/badge/writes-dry--run%20by%20default-4b5563?style=flat-square">
  <img alt="Agent friendly" src="https://img.shields.io/badge/agent-friendly-2563eb?style=flat-square">
</p>

<p align="center">
  <a href="README.md">English</a> | 简体中文
</p>

## 功能概览

`confluence-cli` 是一个小而专注的 Confluence Cloud CLI。它面向需要可预测 Confluence 读取和更安全页面写入的人类用户与 Agent，而不是封装整个 REST API。

- 为操作命令提供稳定的 JSON 信封。
- 支持搜索、页面读取、空间列表、页面创建和页面更新。
- 写入命令默认 dry-run，只有添加 `--execute` 才会真正写入。
- 将 Markdown 输入转换为 Confluence storage 格式。
- 支持原始 Confluence storage XML，以便保留布局或宏。
- 提供配套 Skills 包，指导 Agent 安全使用 CLI。

## 安装

安装最新 release 二进制文件：

```bash
curl -fsSL https://raw.githubusercontent.com/laipz8200/confluence-cli/main/install.sh | sh
```

安装脚本支持 Linux x86_64、Linux arm64、macOS x86_64 和 macOS arm64
release 产物。默认安装到 `~/.local/bin`。可以通过环境变量覆盖安装目录或版本：

```bash
curl -fsSL https://raw.githubusercontent.com/laipz8200/confluence-cli/main/install.sh | CONFLUENCE_CLI_INSTALL_DIR=/usr/local/bin sh
curl -fsSL https://raw.githubusercontent.com/laipz8200/confluence-cli/main/install.sh | CONFLUENCE_CLI_VERSION=0.1.0 sh
```

需要本地构建时，可以从源码检出目录安装：

```bash
cargo install --path .
```

用于本地开发构建：

```bash
cargo build --release
```

## Agent Skills 包

如果你正在使用 Agent，`confluence-cli config init` 会在保存配置后询问是否安装配套 Skills 包。按 Enter 安装，或输入 `n` 跳过。

`config init` 内置安装流程会运行下方同样的 `npx skills add ...` 命令，因此需要本机 `PATH` 中可用 `npx`。

手动安装或重新安装：

```bash
npx skills add laipz8200/confluence-cli --skill confluence-cli
```

该 skill 会指导 Agent 先使用 dry-run，总结计划写入的内容，并在添加 `--execute` 之前请求明确批准。

## 快速开始

创建配置：

```bash
confluence-cli config init
```

初始化流程会验证 Confluence API 凭据，按名称列出可访问的空间，并允许你选择一个可选的默认空间。如果没有可访问空间，配置仍会被保存，但不会设置默认空间。

列出空间：

```bash
confluence-cli space list
```

按文本搜索：

```bash
confluence-cli search --query "deploy"
```

读取页面：

```bash
confluence-cli page get --page-id 123456
```

安全地草拟一次页面写入：

```bash
confluence-cli page create --space-key ENG --title "Release Notes" --body-file release-notes.md
```

上面的 create 命令会返回 dry-run JSON，不会写入 Confluence。只有在审阅 dry-run 输出后才添加 `--execute`。

## 配置

默认配置路径是：

```text
~/.config/confluence-cli/config.toml
```

也可以为单条命令覆盖配置路径：

```bash
CONFLUENCE_CLI_CONFIG=/path/to/config.toml confluence-cli space list
```

API token 会以明文存储。在 Unix 平台上，`confluence-cli` 会用 `0600` 权限写入配置文件。请使用专用 Atlassian API token，并确保配置文件不要进入源码控制。

`default_space` 是可选项。当配置省略它时，命令会继续把 JSON 输出写入 stdout，并在加载配置时向 stderr 打印警告。

## 命令地图

| 命令 | 用途 | 说明 |
| --- | --- | --- |
| `confluence-cli config init` | 创建本地配置文件 | 交互式设置 |
| `confluence-cli space list` | 列出可访问空间 | JSON 输出 |
| `confluence-cli search --query "deploy"` | 按文本搜索 | 构造保守的 CQL 查询 |
| `confluence-cli search --cql 'space = ENG and text ~ "deploy"'` | 运行显式 CQL | 用于高级搜索 |
| `confluence-cli page get --page-id 123456` | 读取页面元数据和正文 | 使用页面 ID，而不是标题 |
| `confluence-cli page create --space-key ENG --title "New Page" --body-file page.md` | 预览页面创建 | 默认 dry-run |
| `confluence-cli page create --space-key ENG --title "New Page" --body-file page.md --parent-id 123456` | 预览子页面创建 | 默认 dry-run |
| `confluence-cli page update --page-id 123456 --title "Updated Page" --body-file page.md` | 预览页面更新 | 默认 dry-run |

## 安全写入

写入命令有意设计为两步：

1. 不带 `--execute` 运行 create 或 update 命令。
2. 检查返回的 dry-run JSON。
3. 只有在批准后，才带上 `--execute` 重新运行同一条命令。

```bash
confluence-cli page update --page-id 123456 --title "Updated Page" --body-file page.md
confluence-cli page update --page-id 123456 --title "Updated Page" --body-file page.md --execute
```

更新要求提供 `--page-id`；该 CLI 不会按标题更新页面。在 update dry-run 期间，CLI 会读取当前页面版本，以便最终写入时使用下一个 Confluence 版本号。

## 正文格式

默认情况下，正文格式会从文件名推断：

| 文件名 | 表示形式 |
| --- | --- |
| `page.md` | Markdown 转换为 Confluence storage HTML |
| `page.storage` | 原始 Confluence storage XML |
| `page.storage.xml` | 原始 Confluence storage XML |
| `page.xml` | 原始 Confluence storage XML |

对于非标准文件名，可以使用 `--body-representation storage` 或 `--body-representation markdown` 覆盖推断结果。

Storage 模式会原样发送文件内容。当 Markdown 无法表示页面时使用它，例如 Confluence 布局或 `<ac:structured-macro>` 这类宏。

## JSON 契约

成功的操作命令会返回稳定信封：

```json
{
  "ok": true,
  "command": "page.update",
  "dry_run": true,
  "data": {}
}
```

应用错误会使用相同形状，并带有稳定的错误代码：

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

`--help`、`--version` 和 clap 参数解析错误可能会打印普通 CLI 文本。

## 开发

运行测试套件：

```bash
cargo test --locked
```

打开 pull request 前运行 Clippy：

```bash
cargo clippy --locked --all-targets --all-features -- -D warnings
```

## 许可证

基于 Apache License, Version 2.0 授权。详见 [LICENSE](LICENSE)。
