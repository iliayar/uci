use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Step {
    RunContainer(RunContainerConfig),
    BuildImage(BuildImageConfig),
    RunShell(RunShellConfig),
    Request(RequestConfig),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Pipeline {
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildImageConfig {
    pub image: String,
    pub tag: Option<String>,
    pub source: Option<BuildImageConfigSource>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildImageConfigSource {
    pub dockerfile: Option<String>,
    pub path: BuildImageConfigSourcePath,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum BuildImageConfigSourcePath {
    Directory(String),
    Tar(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunShellConfig {
    pub script: String,
    pub docker_image: Option<String>,
    pub interpreter: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RequestConfig {
    pub url: String,
    pub method: RequestMethod,
    pub body: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RequestMethod {
    Post, Get,
}
