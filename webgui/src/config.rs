#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub ws_runner_url: String,
    pub runner_url: String,
    pub token: Option<String>,
}

impl runner_client::RunnerClientConfig for Config {
    fn ws_runner_url(&self) -> Option<&str> {
        Some(self.ws_runner_url.as_str())
    }

    fn runner_url(&self) -> Option<&str> {
        Some(self.runner_url.as_str())
    }

    fn token(&self) -> Option<&str> {
        self.token.as_ref().map(|s| s.as_str())
    }
}
