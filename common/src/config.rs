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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Pipeline {
    pub jobs: HashMap<String, Job>,
    pub links: HashMap<String, String>,
    pub networks: Vec<String>,
    pub volumes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Job {
    pub needs: Vec<String>,
    pub steps: Vec<Step>,
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
    pub volumes: HashMap<String, String>,
    pub networks: Vec<String>,
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
