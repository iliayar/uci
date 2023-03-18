use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Step {
    RunContainer(RunContainerConfig),
    BuildImage(BuildImageConfig),
    RunShell(RunShellConfig),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub steps: Vec<Step>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunContainerConfig {
    pub name: String,
    pub image: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildImagePullConfig {
    pub image: String,
    pub tag: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildImagePathConfig {
    pub path: String,
    pub dockerfile: Option<String>,
    pub tag: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildImageArchiveConfig {
    pub tar_path: String,
    pub dockerfile: Option<String>,
    pub tag: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum BuildImageSource {
    Path(BuildImagePathConfig),
    Archive(BuildImageArchiveConfig),
    Pull(BuildImagePullConfig),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildImageConfig {
    pub source: BuildImageSource,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunShellConfig {
    pub script: String,
    pub docker_image: Option<String>,
}
