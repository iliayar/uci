#[derive(Clone)]
pub struct Config {
    pub runner_url: String,
}

impl runner_client::RunnerClientConfig for Config {
    fn runner_url(&self) -> Option<&str> {
        Some(self.runner_url.as_str())
    }

    fn token(&self) -> Option<&str> {
        None
    }
}
