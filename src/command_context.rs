use crate::client::ConfluenceClient;
use crate::config::load_default_config;
use crate::error::AppError;

pub struct CommandContext {
    client: ConfluenceClient,
}

impl CommandContext {
    pub fn load() -> Result<Self, AppError> {
        let config = load_default_config()?;
        let client = ConfluenceClient::new(config)?;

        Ok(Self { client })
    }

    pub fn client(&self) -> &ConfluenceClient {
        &self.client
    }
}
