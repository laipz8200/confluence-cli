use crate::client::{ConfluenceClient, Space};
use crate::commands::{CommandOutput, CommandResult};
use crate::config::{config_path, save_config, Config};
use crate::error::{AppError, ErrorCode};
use clap::Args;
use std::io::{self, Write};

pub const COMMAND: &str = "config.init";

#[derive(Debug, Args)]
pub struct ConfigInitArgs {}

pub async fn run(_args: ConfigInitArgs) -> CommandResult {
    let site_url = prompt("Confluence site URL")?;
    let email = prompt("Email")?;
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

    Ok(CommandOutput::text(
        COMMAND,
        format!(
            "Congratulations, setup is complete.\nConfig saved to: {}",
            path.display()
        ),
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

fn choose_default_space(spaces: &[Space]) -> Result<Option<String>, AppError> {
    let stderr = io::stderr();
    let mut output = stderr.lock();
    let stdin = io::stdin();
    let mut input = stdin.lock();

    choose_default_space_with_io(spaces, &mut input, &mut output)
}

fn choose_default_space_with_io(
    spaces: &[Space],
    input: &mut impl io::BufRead,
    prompt_output: &mut impl Write,
) -> Result<Option<String>, AppError> {
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
}
