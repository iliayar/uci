use std::{collections::HashMap, path::PathBuf};

use super::LoadConfigError;

const SERVICES_CONFIG: &str = "services.yaml";

#[derive(Debug)]
pub struct Services {
    services: HashMap<String, Service>,
}

#[derive(Debug)]
pub struct Service {
    id: String,
    global: bool,
    build: Option<Build>,
    image: Option<String>,
}

#[derive(Debug)]
struct Build {
    repo: String,
    dockerfile: Option<String>,
    context: Option<String>,
}

impl Services {
    pub async fn load(project_root: PathBuf) -> Result<Services, LoadConfigError> {
        raw::parse(project_root.join(SERVICES_CONFIG)).await
    }

    pub fn get(&self, service: &str) -> Option<&Service> {
        self.services.get(service)
    }
}

impl Service {
    pub fn get_build_config(
        &self,
        project_id: &str,
        config: &super::ServiceConfig,
    ) -> Option<common::BuildImageConfig> {
        let image = self.get_image_name(project_id);
        let source = self
            .build
            .as_ref()
            .map(|build| common::BuildImageConfigSource {
                dockerfile: build.dockerfile.clone(),
                path: common::BuildImageConfigSourcePath::Directory(
                    config
                        .repos_path
                        .join(build.repo.clone())
                        .to_string_lossy()
                        .to_string(),
                ),
                context: build.context.clone(),
            });

        Some(common::BuildImageConfig {
            image,
            tag: None, // FIXME: Specify somewhere
            source,
        })
    }

    pub fn get_run_config(&self, project_id: &str) -> Option<common::RunContainerConfig> {
        let image = self.get_image_name(project_id);
        let name = self.get_container_name(project_id);

        Some(common::RunContainerConfig { name, image })
    }

    pub fn get_stop_config(&self, project_id: &str) -> Option<common::StopContainerConfig> {
        let name = self.get_container_name(project_id);

        Some(common::StopContainerConfig { name })
    }

    fn get_image_name(&self, project_id: &str) -> String {
        if let Some(image) = &self.image {
            // Will pull specified image
            String::from(image)
        } else if self.global {
            // Image name is service name
            String::from(&self.id)
        } else {
            // Image name is scoped under project
            format!("{}_{}", project_id, self.id)
        }
    }

    fn get_container_name(&self, project_id: &str) -> String {
        if self.global {
            // Container name is service name
            String::from(&self.id)
        } else {
            // Container name is scoped under project
            format!("{}_{}", project_id, self.id)
        }
    }
}

mod raw {
    use std::{collections::HashMap, path::PathBuf};

    use serde::{Deserialize, Serialize};

    use crate::lib::{config, utils};

    #[derive(Serialize, Deserialize)]
    struct Services {
        services: HashMap<String, Service>,
    }

    #[derive(Serialize, Deserialize)]
    struct Service {
        #[serde(default = "default_global")]
        global: bool,
        build: Option<Build>,
        image: Option<String>,
    }

    #[derive(Serialize, Deserialize)]
    struct Build {
        repo: String,
        dockerfile: Option<String>,
        context: Option<String>,
    }

    fn default_global() -> bool {
        false
    }

    impl TryFrom<Services> for super::Services {
        type Error = super::LoadConfigError;

        fn try_from(value: Services) -> Result<Self, Self::Error> {
            let services: Result<HashMap<_, _>, super::LoadConfigError> = value
                .services
                .into_iter()
                .map(|(k, v)| {
                    let build = if let Some(build) = v.build {
                        Some(build.try_into()?)
                    } else {
                        None
                    };

                    Ok((
                        k.clone(),
                        super::Service {
                            id: k.clone(),
                            global: v.global,
                            image: v.image,
                            build,
                        },
                    ))
                })
                .collect();
            let services = services?;

            Ok(super::Services { services })
        }
    }

    impl TryFrom<Build> for super::Build {
        type Error = super::LoadConfigError;

        fn try_from(value: Build) -> Result<Self, Self::Error> {
            Ok(super::Build {
                repo: value.repo,
                dockerfile: value.dockerfile,
                context: value.context,
            })
        }
    }

    pub async fn parse(path: PathBuf) -> Result<super::Services, super::LoadConfigError> {
        config::utils::load_file::<Services, _>(path).await
    }
}
