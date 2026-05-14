use crate::client::{ConfluenceClient, Space};
use crate::commands::{CommandOutput, CommandResult};
use crate::config::{config_path, save_config, Config};
use crate::error::{AppError, ErrorCode};
use clap::Args;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::Command;

pub const COMMAND: &str = "config.init";
const SKILLS_INSTALL_PROGRAM: &str = "npx";
const SKILLS_INSTALL_ARGS: [&str; 5] = [
    "skills",
    "add",
    "laipz8200/confluence-cli",
    "--skill",
    "confluence-cli",
];
const ANSI_RESET: &str = "\u{1b}[0m";
const ANSI_BOLD: &str = "\u{1b}[1m";
const ANSI_DIM: &str = "\u{1b}[2m";
const ANSI_GREEN: &str = "\u{1b}[32m";
const ANSI_CYAN: &str = "\u{1b}[36m";

#[derive(Debug, Args)]
pub struct ConfigInitArgs {}

pub async fn run(_args: ConfigInitArgs) -> CommandResult {
    let site_url = prompt(ConfigInitStep::site_url(), "Confluence site URL")?;
    let email = prompt(ConfigInitStep::email(), "Email")?;
    let api_token = read_api_token()?;

    let mut config = Config {
        site_url,
        email,
        api_token,
        default_space: None,
    };
    let client = ConfluenceClient::new(config.clone())?;
    let spaces = client
        .list_all_spaces()
        .await
        .map_err(api_verification_error)?;
    config.default_space = choose_default_space(&spaces)?;

    let path = config_path()?;
    save_config(&path, &config)?;
    let skills_install =
        prompt_and_install_skills().map_err(|error| after_config_save_error(error, &path))?;

    let mut message = format!(
        "Congratulations, setup is complete.\nConfig saved to: {}",
        path.display()
    );
    if skills_install == SkillsInstallOutcome::Installed {
        message.push_str("\nAgent Skills package installed.");
    }

    Ok(CommandOutput::text(COMMAND, message))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromptDisplayMode {
    Plain,
    Dynamic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ConfigInitStep {
    number: usize,
    label: &'static str,
    connect_after: bool,
}

impl ConfigInitStep {
    fn site_url() -> Self {
        Self {
            number: 1,
            label: "Confluence site URL",
            connect_after: true,
        }
    }

    fn email() -> Self {
        Self {
            number: 2,
            label: "Email",
            connect_after: true,
        }
    }

    fn api_token() -> Self {
        Self {
            number: 3,
            label: "API token",
            connect_after: true,
        }
    }

    fn default_space() -> Self {
        Self {
            number: 4,
            label: "Default space",
            connect_after: true,
        }
    }

    fn agent_skills() -> Self {
        Self {
            number: 5,
            label: "Agent Skills package",
            connect_after: false,
        }
    }
}

fn prompt_display_mode() -> PromptDisplayMode {
    if io::stdin().is_terminal() && io::stderr().is_terminal() {
        PromptDisplayMode::Dynamic
    } else {
        PromptDisplayMode::Plain
    }
}

fn prompt(step: ConfigInitStep, label: &str) -> Result<String, AppError> {
    let display_mode = prompt_display_mode();
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stderr = io::stderr();
    let mut prompt_output = stderr.lock();
    prompt_step_with_io(step, label, &mut input, &mut prompt_output, display_mode)
}

fn read_api_token() -> Result<String, AppError> {
    let step = ConfigInitStep::api_token();
    if prompt_display_mode() == PromptDisplayMode::Plain {
        return match rpassword::prompt_password("API token: ") {
            Ok(api_token) => Ok(api_token.trim().to_string()),
            Err(source) if is_tty_unavailable(&source) => prompt(step, "API token"),
            Err(source) => Err(AppError::new(
                crate::error::ErrorCode::ConfigInvalid,
                format!("Failed to read API token: {source}"),
            )),
        };
    }

    {
        let stderr = io::stderr();
        let mut prompt_output = stderr.lock();
        render_step_active(step, &mut prompt_output)?;
    }

    match rpassword::prompt_password(format!("  {ANSI_DIM}API token:{ANSI_RESET} ")) {
        Ok(api_token) => {
            let stderr = io::stderr();
            let mut prompt_output = stderr.lock();
            clear_rendered_lines(&mut prompt_output, 2)?;
            render_step_complete(step, &mut prompt_output)?;
            Ok(api_token.trim().to_string())
        }
        Err(source) if is_tty_unavailable(&source) => {
            let stdin = io::stdin();
            let mut input = stdin.lock();
            let stderr = io::stderr();
            let mut prompt_output = stderr.lock();
            clear_rendered_lines(&mut prompt_output, 1)?;
            prompt_step_with_io(
                step,
                "API token",
                &mut input,
                &mut prompt_output,
                PromptDisplayMode::Dynamic,
            )
        }
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

fn choose_default_space(spaces: &[Space]) -> Result<Option<String>, AppError> {
    let display_mode = prompt_display_mode();
    let stderr = io::stderr();
    let mut output = stderr.lock();
    let stdin = io::stdin();
    let mut input = stdin.lock();

    choose_default_space_with_io_and_display(spaces, &mut input, &mut output, display_mode)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkillsInstallOutcome {
    Installed,
    Skipped,
}

fn prompt_and_install_skills() -> Result<SkillsInstallOutcome, AppError> {
    let display_mode = prompt_display_mode();
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stderr = io::stderr();
    let mut prompt_output = stderr.lock();

    prompt_and_install_skills_with_io_and_display(
        &mut input,
        &mut prompt_output,
        run_skills_install_command,
        display_mode,
    )
}

#[cfg(test)]
fn prompt_and_install_skills_with_io(
    input: &mut impl io::BufRead,
    prompt_output: &mut impl Write,
    mut installer: impl FnMut(&str, &[&str]) -> Result<(), AppError>,
) -> Result<SkillsInstallOutcome, AppError> {
    prompt_and_install_skills_with_io_and_display(
        input,
        prompt_output,
        &mut installer,
        PromptDisplayMode::Plain,
    )
}

fn prompt_and_install_skills_with_io_and_display(
    input: &mut impl io::BufRead,
    prompt_output: &mut impl Write,
    mut installer: impl FnMut(&str, &[&str]) -> Result<(), AppError>,
    display_mode: PromptDisplayMode,
) -> Result<SkillsInstallOutcome, AppError> {
    if display_mode == PromptDisplayMode::Dynamic {
        return prompt_and_install_skills_dynamic(input, prompt_output, installer);
    }

    loop {
        write!(
            prompt_output,
            "Install the companion Agent Skills package now? [Y/n]: "
        )
        .map_err(prompt_write_error)?;
        prompt_output.flush().map_err(|source| {
            AppError::new(
                ErrorCode::ConfigInvalid,
                format!("Failed to flush prompt: {source}"),
            )
        })?;

        let mut value = String::new();
        let bytes_read = input.read_line(&mut value).map_err(|source| {
            AppError::new(
                ErrorCode::ConfigInvalid,
                format!("Failed to read Agent Skills install choice: {source}"),
            )
        })?;
        if bytes_read == 0 {
            return Ok(SkillsInstallOutcome::Skipped);
        }

        let value = value.trim();
        if value.is_empty() || value.eq_ignore_ascii_case("y") || value.eq_ignore_ascii_case("yes")
        {
            installer(SKILLS_INSTALL_PROGRAM, &SKILLS_INSTALL_ARGS)?;
            return Ok(SkillsInstallOutcome::Installed);
        }

        if value.eq_ignore_ascii_case("n") || value.eq_ignore_ascii_case("no") {
            return Ok(SkillsInstallOutcome::Skipped);
        }

        writeln!(prompt_output, "Please answer y or n.").map_err(prompt_write_error)?;
    }
}

fn prompt_and_install_skills_dynamic(
    input: &mut impl io::BufRead,
    prompt_output: &mut impl Write,
    mut installer: impl FnMut(&str, &[&str]) -> Result<(), AppError>,
) -> Result<SkillsInstallOutcome, AppError> {
    let step = ConfigInitStep::agent_skills();
    let mut error_rendered = false;

    render_step_active(step, prompt_output)?;
    loop {
        write!(
            prompt_output,
            "  {ANSI_DIM}Install companion Agent Skills package? [Y/n]:{ANSI_RESET} "
        )
        .map_err(prompt_write_error)?;
        flush_prompt_output(prompt_output)?;

        let mut value = String::new();
        let bytes_read = input.read_line(&mut value).map_err(|source| {
            AppError::new(
                ErrorCode::ConfigInvalid,
                format!("Failed to read Agent Skills install choice: {source}"),
            )
        })?;
        if bytes_read == 0 {
            let lines = 1 + usize::from(error_rendered) + 1;
            clear_rendered_lines(prompt_output, lines)?;
            render_step_complete(step, prompt_output)?;
            return Ok(SkillsInstallOutcome::Skipped);
        }

        let value = value.trim();
        if value.is_empty() || value.eq_ignore_ascii_case("y") || value.eq_ignore_ascii_case("yes")
        {
            let lines = 1 + usize::from(error_rendered) + 1;
            clear_rendered_lines(prompt_output, lines)?;
            installer(SKILLS_INSTALL_PROGRAM, &SKILLS_INSTALL_ARGS)?;
            render_step_complete(step, prompt_output)?;
            return Ok(SkillsInstallOutcome::Installed);
        }

        if value.eq_ignore_ascii_case("n") || value.eq_ignore_ascii_case("no") {
            let lines = 1 + usize::from(error_rendered) + 1;
            clear_rendered_lines(prompt_output, lines)?;
            render_step_complete(step, prompt_output)?;
            return Ok(SkillsInstallOutcome::Skipped);
        }

        let lines = usize::from(error_rendered) + 1;
        clear_rendered_lines(prompt_output, lines)?;
        writeln!(
            prompt_output,
            "  {ANSI_DIM}Please answer y or n.{ANSI_RESET}"
        )
        .map_err(prompt_write_error)?;
        error_rendered = true;
    }
}

fn run_skills_install_command(program: &str, args: &[&str]) -> Result<(), AppError> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|source| {
            AppError::new(
                ErrorCode::InternalError,
                format!(
                    "Failed to start Agent Skills installer `{}`: {source}",
                    skills_install_command()
                ),
            )
        })?;

    if status.success() {
        return Ok(());
    }

    Err(AppError::new(
        ErrorCode::InternalError,
        format!(
            "Agent Skills installer `{}` exited with {status}.",
            skills_install_command()
        ),
    ))
}

fn skills_install_command() -> String {
    format!(
        "{} {}",
        SKILLS_INSTALL_PROGRAM,
        SKILLS_INSTALL_ARGS.join(" ")
    )
}

fn after_config_save_error(mut error: AppError, path: &Path) -> AppError {
    error.message = format!("Config saved to: {}. {}", path.display(), error.message);
    error
}

#[cfg(test)]
fn choose_default_space_with_io(
    spaces: &[Space],
    input: &mut impl io::BufRead,
    prompt_output: &mut impl Write,
) -> Result<Option<String>, AppError> {
    choose_default_space_with_io_and_display(spaces, input, prompt_output, PromptDisplayMode::Plain)
}

fn choose_default_space_with_io_and_display(
    spaces: &[Space],
    input: &mut impl io::BufRead,
    prompt_output: &mut impl Write,
    display_mode: PromptDisplayMode,
) -> Result<Option<String>, AppError> {
    if display_mode == PromptDisplayMode::Dynamic {
        return choose_default_space_dynamic(spaces, input, prompt_output);
    }

    if spaces.is_empty() {
        writeln!(
            prompt_output,
            "No accessible spaces found. Saving config without a default_space."
        )
        .map_err(prompt_write_error)?;
        return Ok(None);
    }

    writeln!(prompt_output, "Accessible spaces:").map_err(prompt_write_error)?;
    for (index, space) in spaces.iter().enumerate() {
        writeln!(
            prompt_output,
            "{}. {} ({})",
            index + 1,
            space.name,
            space.key
        )
        .map_err(prompt_write_error)?;
    }

    loop {
        let value = prompt_with_io(
            "Default space number (press Enter to skip)",
            input,
            prompt_output,
        )?;
        if value.is_empty() {
            return Ok(None);
        }

        let Ok(selection) = value.parse::<usize>() else {
            writeln!(
                prompt_output,
                "Please enter a number from 1 to {}, or press Enter to skip.",
                spaces.len()
            )
            .map_err(prompt_write_error)?;
            continue;
        };

        if let Some(space) = spaces.get(selection.saturating_sub(1)) {
            return Ok(Some(space.key.clone()));
        }

        writeln!(
            prompt_output,
            "Please enter a number from 1 to {}, or press Enter to skip.",
            spaces.len()
        )
        .map_err(prompt_write_error)?;
    }
}

fn choose_default_space_dynamic(
    spaces: &[Space],
    input: &mut impl io::BufRead,
    prompt_output: &mut impl Write,
) -> Result<Option<String>, AppError> {
    let step = ConfigInitStep::default_space();
    render_step_active(step, prompt_output)?;

    if spaces.is_empty() {
        clear_rendered_lines(prompt_output, 1)?;
        render_step_complete(step, prompt_output)?;
        return Ok(None);
    }

    writeln!(prompt_output, "  {ANSI_DIM}Accessible spaces{ANSI_RESET}")
        .map_err(prompt_write_error)?;
    for (index, space) in spaces.iter().enumerate() {
        writeln!(
            prompt_output,
            "    {ANSI_DIM}{}.{ANSI_RESET} {} {ANSI_DIM}({}){ANSI_RESET}",
            index + 1,
            space.name,
            space.key
        )
        .map_err(prompt_write_error)?;
    }

    let base_lines = 1 + 1 + spaces.len();
    let mut error_rendered = false;
    loop {
        write!(
            prompt_output,
            "  {ANSI_DIM}Select default space [Enter to skip]:{ANSI_RESET} "
        )
        .map_err(prompt_write_error)?;
        flush_prompt_output(prompt_output)?;

        let mut value = String::new();
        input.read_line(&mut value).map_err(|source| {
            AppError::new(
                ErrorCode::ConfigInvalid,
                format!("Failed to read Default space number (press Enter to skip): {source}"),
            )
        })?;
        let value = value.trim();

        if value.is_empty() {
            let lines = base_lines + usize::from(error_rendered) + 1;
            clear_rendered_lines(prompt_output, lines)?;
            render_step_complete(step, prompt_output)?;
            return Ok(None);
        }

        if let Ok(selection) = value.parse::<usize>() {
            if let Some(space) = spaces.get(selection.saturating_sub(1)) {
                let lines = base_lines + usize::from(error_rendered) + 1;
                clear_rendered_lines(prompt_output, lines)?;
                render_step_complete(step, prompt_output)?;
                return Ok(Some(space.key.clone()));
            }
        }

        let lines = usize::from(error_rendered) + 1;
        clear_rendered_lines(prompt_output, lines)?;
        writeln!(
            prompt_output,
            "  {ANSI_DIM}Enter a number from 1 to {}, or press Enter to skip.{ANSI_RESET}",
            spaces.len()
        )
        .map_err(prompt_write_error)?;
        error_rendered = true;
    }
}

fn prompt_step_with_io(
    step: ConfigInitStep,
    label: &str,
    input: &mut impl io::BufRead,
    prompt_output: &mut impl Write,
    display_mode: PromptDisplayMode,
) -> Result<String, AppError> {
    if display_mode == PromptDisplayMode::Plain {
        return prompt_with_io(label, input, prompt_output);
    }

    render_step_active(step, prompt_output)?;
    write!(prompt_output, "  {ANSI_DIM}{label}:{ANSI_RESET} ").map_err(|source| {
        AppError::new(
            crate::error::ErrorCode::ConfigInvalid,
            format!("Failed to write prompt: {source}"),
        )
    })?;
    flush_prompt_output(prompt_output)?;

    let mut value = String::new();
    input.read_line(&mut value).map_err(|source| {
        AppError::new(
            crate::error::ErrorCode::ConfigInvalid,
            format!("Failed to read {label}: {source}"),
        )
    })?;
    clear_rendered_lines(prompt_output, 2)?;
    render_step_complete(step, prompt_output)?;

    Ok(value.trim().to_string())
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
    flush_prompt_output(prompt_output)?;
    let mut value = String::new();
    input.read_line(&mut value).map_err(|source| {
        AppError::new(
            crate::error::ErrorCode::ConfigInvalid,
            format!("Failed to read {label}: {source}"),
        )
    })?;
    Ok(value.trim().to_string())
}

fn render_step_active(
    step: ConfigInitStep,
    prompt_output: &mut impl Write,
) -> Result<(), AppError> {
    writeln!(
        prompt_output,
        "{ANSI_CYAN}●{ANSI_RESET} {ANSI_BOLD}{}. {}{ANSI_RESET}",
        step.number, step.label
    )
    .map_err(prompt_write_error)?;
    flush_prompt_output(prompt_output)
}

fn render_step_complete(
    step: ConfigInitStep,
    prompt_output: &mut impl Write,
) -> Result<(), AppError> {
    writeln!(
        prompt_output,
        "{ANSI_GREEN}✔{ANSI_RESET} {ANSI_DIM}{}.{ANSI_RESET} {}",
        step.number, step.label
    )
    .map_err(prompt_write_error)?;
    if step.connect_after {
        writeln!(prompt_output, "{ANSI_DIM}│{ANSI_RESET}").map_err(prompt_write_error)?;
    }
    flush_prompt_output(prompt_output)
}

fn clear_rendered_lines(prompt_output: &mut impl Write, count: usize) -> Result<(), AppError> {
    for _ in 0..count {
        write!(prompt_output, "\u{1b}[1A\u{1b}[2K").map_err(prompt_write_error)?;
    }
    flush_prompt_output(prompt_output)
}

fn flush_prompt_output(prompt_output: &mut impl Write) -> Result<(), AppError> {
    prompt_output.flush().map_err(|source| {
        AppError::new(
            crate::error::ErrorCode::ConfigInvalid,
            format!("Failed to flush prompt: {source}"),
        )
    })
}

fn prompt_write_error(source: io::Error) -> AppError {
    AppError::new(
        ErrorCode::ConfigInvalid,
        format!("Failed to write prompt: {source}"),
    )
}

fn api_verification_error(error: AppError) -> AppError {
    AppError {
        code: error.code,
        message: format!(
            "Failed to verify Confluence API access. Run `confluence-cli config init` again to retry. {}",
            error.message
        ),
        retryable: error.retryable,
        details: error.details,
    }
}

#[cfg(test)]
mod tests {
    use crate::client::Space;

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

    #[test]
    fn prompt_step_dynamic_display_clears_input_and_marks_step_done() {
        let mut input = " https://example.atlassian.net \n".as_bytes();
        let mut prompt_output = Vec::new();

        let value = super::prompt_step_with_io(
            super::ConfigInitStep::site_url(),
            "Confluence site URL",
            &mut input,
            &mut prompt_output,
            super::PromptDisplayMode::Dynamic,
        )
        .unwrap();

        assert_eq!(value, "https://example.atlassian.net");
        assert_eq!(
            String::from_utf8(prompt_output).unwrap(),
            concat!(
                "\u{1b}[36m●\u{1b}[0m \u{1b}[1m1. Confluence site URL\u{1b}[0m\n",
                "  \u{1b}[2mConfluence site URL:\u{1b}[0m ",
                "\u{1b}[1A\u{1b}[2K\u{1b}[1A\u{1b}[2K",
                "\u{1b}[32m✔\u{1b}[0m \u{1b}[2m1.\u{1b}[0m Confluence site URL\n",
                "\u{1b}[2m│\u{1b}[0m\n",
            )
        );
    }

    #[test]
    fn default_space_selection_returns_selected_space_key() {
        let spaces = vec![
            Space {
                id: "space-eng".to_string(),
                key: "ENG".to_string(),
                name: "Engineering".to_string(),
            },
            Space {
                id: "space-docs".to_string(),
                key: "DOCS".to_string(),
                name: "Documentation".to_string(),
            },
        ];
        let mut input = "2\n".as_bytes();
        let mut prompt_output = Vec::new();

        let selected =
            super::choose_default_space_with_io(&spaces, &mut input, &mut prompt_output).unwrap();

        assert_eq!(selected.as_deref(), Some("DOCS"));
        let prompt = String::from_utf8(prompt_output).unwrap();
        assert!(prompt.contains("Engineering"));
        assert!(prompt.contains("Documentation"));
        assert!(prompt.contains("Default space number"));
    }

    #[test]
    fn default_space_selection_allows_skipping() {
        let spaces = vec![Space {
            id: "space-eng".to_string(),
            key: "ENG".to_string(),
            name: "Engineering".to_string(),
        }];
        let mut input = "\n".as_bytes();
        let mut prompt_output = Vec::new();

        let selected =
            super::choose_default_space_with_io(&spaces, &mut input, &mut prompt_output).unwrap();

        assert_eq!(selected, None);
    }

    #[test]
    fn default_space_selection_dynamic_display_clears_space_list_after_selection() {
        let spaces = vec![
            Space {
                id: "space-eng".to_string(),
                key: "ENG".to_string(),
                name: "Engineering".to_string(),
            },
            Space {
                id: "space-docs".to_string(),
                key: "DOCS".to_string(),
                name: "Documentation".to_string(),
            },
        ];
        let mut input = "2\n".as_bytes();
        let mut prompt_output = Vec::new();

        let selected = super::choose_default_space_with_io_and_display(
            &spaces,
            &mut input,
            &mut prompt_output,
            super::PromptDisplayMode::Dynamic,
        )
        .unwrap();

        assert_eq!(selected.as_deref(), Some("DOCS"));
        assert_eq!(
            String::from_utf8(prompt_output).unwrap(),
            concat!(
                "\u{1b}[36m●\u{1b}[0m \u{1b}[1m4. Default space\u{1b}[0m\n",
                "  \u{1b}[2mAccessible spaces\u{1b}[0m\n",
                "    \u{1b}[2m1.\u{1b}[0m Engineering \u{1b}[2m(ENG)\u{1b}[0m\n",
                "    \u{1b}[2m2.\u{1b}[0m Documentation \u{1b}[2m(DOCS)\u{1b}[0m\n",
                "  \u{1b}[2mSelect default space [Enter to skip]:\u{1b}[0m ",
                "\u{1b}[1A\u{1b}[2K\u{1b}[1A\u{1b}[2K\u{1b}[1A\u{1b}[2K\u{1b}[1A\u{1b}[2K\u{1b}[1A\u{1b}[2K",
                "\u{1b}[32m✔\u{1b}[0m \u{1b}[2m4.\u{1b}[0m Default space\n",
                "\u{1b}[2m│\u{1b}[0m\n",
            )
        );
    }

    #[test]
    fn skills_install_prompt_defaults_enter_to_install() {
        let mut input = "\n".as_bytes();
        let mut prompt_output = Vec::new();
        let mut calls = Vec::new();

        let outcome = super::prompt_and_install_skills_with_io(
            &mut input,
            &mut prompt_output,
            |program, args| {
                calls.push((
                    program.to_string(),
                    args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>(),
                ));
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(outcome, super::SkillsInstallOutcome::Installed);
        assert_eq!(
            calls,
            vec![(
                "npx".to_string(),
                vec![
                    "skills".to_string(),
                    "add".to_string(),
                    "laipz8200/confluence-cli".to_string(),
                    "--skill".to_string(),
                    "confluence-cli".to_string(),
                ],
            )]
        );
        assert!(String::from_utf8(prompt_output)
            .unwrap()
            .contains("Install the companion Agent Skills package now? [Y/n]"));
    }

    #[test]
    fn skills_install_prompt_skips_when_user_says_no() {
        let mut input = "n\n".as_bytes();
        let mut prompt_output = Vec::new();

        let outcome =
            super::prompt_and_install_skills_with_io(&mut input, &mut prompt_output, |_, _| {
                panic!("installer should not run when user says no");
            })
            .unwrap();

        assert_eq!(outcome, super::SkillsInstallOutcome::Skipped);
    }

    #[test]
    fn skills_install_prompt_skips_on_eof_without_running_installer() {
        let mut input = "".as_bytes();
        let mut prompt_output = Vec::new();

        let outcome =
            super::prompt_and_install_skills_with_io(&mut input, &mut prompt_output, |_, _| {
                panic!("installer should not run on EOF");
            })
            .unwrap();

        assert_eq!(outcome, super::SkillsInstallOutcome::Skipped);
    }
}
