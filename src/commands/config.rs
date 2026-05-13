use crate::config::{config_path, save_config, Config};
use crate::error::AppError;
use serde_json::json;
use std::io::{self, Write};

pub fn init() -> Result<serde_json::Value, AppError> {
    let site_url = prompt("Confluence site URL")?;
    let email = prompt("Email")?;
    let api_token = read_api_token()?;
    let default_space = prompt("Default space key")?;

    let config = Config {
        site_url,
        email,
        api_token,
        default_space,
    };
    let path = config_path()?;
    save_config(&path, &config)?;

    Ok(json!({
        "path": path,
        "site_url": config.site_url.trim_end_matches('/'),
        "email": config.email,
        "default_space": config.default_space
    }))
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

#[cfg(test)]
mod tests {
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
}
