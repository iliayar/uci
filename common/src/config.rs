use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Step {
    RunContainer(RunContainerConfig),
    BuildImage(BuildImageConfig),
    RunShell(RunShellConfig),
    StopContainer(StopContainerConfig),
    Request(RequestConfig),
    Parallel(ParallelConfig),

    ServiceLogs(ServiceLogsConfig),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Pipeline {
    pub id: String,
    pub jobs: HashMap<String, Job>,
    pub links: HashMap<String, String>,
    pub networks: Vec<String>,
    pub volumes: Vec<String>,
    pub stages: HashMap<String, Stage>,
    pub integrations: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Stage {
    pub overlap_strategy: OverlapStrategy,
    pub repos: Option<StageRepos>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum StageRepos {
    Exact(HashMap<String, RepoLockStrategy>),
    All(RepoLockStrategy),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum RepoLockStrategy {
    Lock,
    Unlock,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum OverlapStrategy {
    /// Keep going runnig pipelines simultaneously
    Ignore,

    /// Interrupt running pipeline
    Displace,

    /// Wait for running pipeline to pass this stage
    Wait,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Job {
    pub enabled: bool,
    pub needs: Vec<String>,
    pub steps: Vec<Step>,
    pub stage: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ParallelConfig {
    pub steps: Vec<Step>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PortMapping {
    pub container_port: u16,
    pub proto: String,
    pub host_port: u16,
    pub host: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RunContainerConfig {
    pub name: String,
    pub image: String,
    pub networks: Vec<String>,
    pub volumes: HashMap<String, String>,
    pub command: Option<Vec<String>>,
    pub ports: Vec<PortMapping>,
    pub restart_policy: String,
    pub env: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServiceLogsConfig {
    pub container: String,
    pub follow: bool,
    pub tail: Option<usize>,
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
    pub volumes: HashMap<String, String>,
    pub networks: Vec<String>,
    pub env: HashMap<String, String>,
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
