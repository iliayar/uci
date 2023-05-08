mod integration;
mod gitlab;

pub use integration::Integrations;

use anyhow::anyhow;

pub fn get_integration(
    key: impl AsRef<str>,
    config: serde_json::Value,
) -> Result<Box<dyn integration::Integration>, anyhow::Error> {
    match key.as_ref() {
        _ => Err(anyhow!("No integration for '{}'", key.as_ref())),
    }
}
