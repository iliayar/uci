use std::collections::HashMap;

use common::utils::run_command_with_output;

use crate::{docker, utils::tempfile};

use super::task;

use anyhow::anyhow;
use log::*;

const DEFAULT_INTERPRETER: &str = "/usr/bin/env";
const DEFAULT_ARGS: [&str; 1] = ["bash"];

#[async_trait::async_trait]
impl task::Task for common::RunShellConfig {
    async fn run(
        self,
        context: &crate::context::Context,
        task_context: &super::TaskContext,
    ) -> Result<(), task::TaskError> {
        let (interpreter, mut args) = get_interpreter_args(self.interpreter)?;
        let script_file = tempfile::TempFile::new_executable(&self.script).await?;

        if let Some(image) = self.docker_image {
            let container_script_file = String::from("/script");
            let task_context_dir = String::from("/tmp/task_context/");

            let mut run_command_builder = docker::RunCommandParamsBuilder::default();
            run_command_builder.image(image);

            let mut command = vec![interpreter];
            command.append(&mut args);
            command.push(container_script_file.clone());
            run_command_builder.command(command);

            let mut mounts = HashMap::new();
            mounts.insert(
                script_file.path.to_string_lossy().to_string(),
                container_script_file.clone(),
            );
            for (link, path) in task_context.links.iter() {
                mounts.insert(
                    path.to_string_lossy().to_string(),
                    format!("{}/{}", task_context_dir, link),
                );
            }
            for (src, dst) in self.volumes {
                mounts.insert(src, dst);
            }
            run_command_builder.mounts(mounts);
	    run_command_builder.networks(self.networks);

            run_command_builder.workdir(Some(task_context_dir));

            context
                .docker()
                .run_command(
                    run_command_builder
                        .build()
                        .map_err(|e| anyhow!("Invalid run commands params: {}", e))?,
                )
                .await?;
        } else {
            let tempdir = crate::utils::tempfile::TempFile::dir().await?;
            info!("Using context directory: {:?}", tempdir.path);

            for (link, path) in task_context.links.iter() {
                tokio::fs::symlink(path, tempdir.path.join(link)).await?;
            }

            let mut command = tokio::process::Command::new(interpreter);
            command.current_dir(&tempdir.path);
            command.args(args);
            command.arg(&script_file.path);

            run_command_with_output(command).await?;
        };

        Ok(())
    }
}

fn get_interpreter_args(
    interpreter: Option<Vec<String>>,
) -> Result<(String, Vec<String>), anyhow::Error> {
    if let Some(command) = interpreter {
        let mut it = command.into_iter();
        let interpreter = it.next().ok_or(anyhow!("Intepreter is not specified"))?;
        let args = it.collect();

        Ok((interpreter, args))
    } else {
        Ok((
            String::from(DEFAULT_INTERPRETER),
            DEFAULT_ARGS.iter().map(|s| String::from(*s)).collect(),
        ))
    }
}
