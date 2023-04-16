use std::path::PathBuf;

use crate::{
    docker::{self, Docker},
    utils::{file_utils, tempfile},
};

use anyhow::anyhow;
use common::state::State;

#[async_trait::async_trait]
impl super::Task for common::BuildImageConfig {
    async fn run(self, state: &State) -> Result<(), anyhow::Error> {
        let docker: &Docker = state.get()?;
        if let Some(source) = self.source {
            let tar_tempfile = match source.path {
                common::BuildImageConfigSourcePath::Directory(path) => {
                    let path: PathBuf = path.into();
                    file_utils::create_temp_tar(path).await?
                }
                common::BuildImageConfigSourcePath::Tar(path) => {
                    tempfile::TempFile::dummy(path.into()).await
                }
            };

            let mut params_builder = docker::BuildParamsBuilder::default();

            params_builder
                .tar_path(tar_tempfile.path.clone())
                .image(self.image);

            if let Some(tag) = self.tag {
                params_builder.tag(tag);
            }

            if let Some(dockerfile) = source.dockerfile {
                params_builder.dockerfile(dockerfile);
            }

            docker
                .build(
                    params_builder
                        .build()
                        .map_err(|e| anyhow!("Invalid build params: {}", e))?,
                )
                .await?;
        } else {
            let mut params_builder = docker::PullParamsBuilder::default();

            params_builder.image(self.image);

            if let Some(tag) = self.tag {
                params_builder.tag(tag);
            }

            docker
                .pull(
                    params_builder
                        .build()
                        .map_err(|e| anyhow!("Invalid pull params: {}", e))?,
                )
                .await?;
        }

        Ok(())
    }
}
