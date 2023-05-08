mod gitlab;
mod integration;
mod telegram;

pub use integration::Integrations;

use anyhow::anyhow;

pub fn get_integration(
    key: impl AsRef<str>,
    config: serde_json::Value,
) -> Result<Box<dyn integration::Integration>, anyhow::Error> {
    match key.as_ref() {
        "gitlab" => Ok(Box::new(gitlab::GitLabIntegration::from_value(config)?)),
        "telegram" => Ok(Box::new(telegram::TelegramIntegration::from_value(config)?)),
        _ => Err(anyhow!("No integration for '{}'", key.as_ref())),
    }
}
