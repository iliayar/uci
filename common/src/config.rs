use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Step {
    RunContainer(RunContainerConfig),
    BuildImage(BuildImageConfig),
    RunShell(RunShellConfig),
    StopContainer(StopContainerConfig),
    Request(RequestConfig),
    Parallel(ParallelConfig),
    Deploy(DeployConfig),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Pipeline {
    pub steps: Vec<Step>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ParallelConfig {
    pub steps: Vec<Step>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeployConfig {
    pub stop_config: StopContainerConfig,
    pub build_config: BuildImageConfig,
    pub run_config: RunContainerConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RunContainerConfig {
    pub name: String,
    pub image: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StopContainerConfig {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BuildImagePullConfig {
    pub image: String,
    pub tag: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BuildImageConfig {
    pub image: String,
    pub tag: Option<String>,
    pub source: Option<BuildImageConfigSource>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BuildImageConfigSource {
    pub dockerfile: Option<String>,
    pub context: Option<String>,
    pub path: BuildImageConfigSourcePath,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum BuildImageConfigSourcePath {
    Directory(String),
    Tar(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RunShellConfig {
    pub script: String,
    pub docker_image: Option<String>,
    pub interpreter: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RequestConfig {
    pub url: String,
    pub method: RequestMethod,
    pub body: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum RequestMethod {
    Post,
    Get,
}
