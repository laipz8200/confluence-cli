use crate::client::ConfluenceClient;
use crate::config::load_default_config;
use crate::error::AppError;

pub struct CommandContext {
    client: ConfluenceClient,
}

impl CommandContext {
    pub fn load() -> Result<Self, AppError> {
        let config = load_default_config()?;
        if config.default_space.is_none() {
            eprintln!(
                "Warning: default_space is not configured. Commands that read the default space require an explicit space."
            );
        }
        let client = ConfluenceClient::new(config)?;

        Ok(Self { client })
    }

    pub fn client(&self) -> &ConfluenceClient {
        &self.client
    }
}
