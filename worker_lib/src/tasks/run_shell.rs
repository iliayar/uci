use std::collections::HashMap;
use std::process::{ExitStatus, Stdio};

use common::state::State;

use crate::docker::{self, Docker};
use crate::executor::Logger;

use common::utils::tempfile;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_stream::{wrappers::LinesStream, StreamExt};

use super::task;

use anyhow::anyhow;
use log::*;

const DEFAULT_INTERPRETER: &str = "/usr/bin/env";
const DEFAULT_ARGS: [&str; 1] = ["bash"];

#[async_trait::async_trait]
impl task::Task for common::RunShellConfig {
    async fn run(self, state: &State) -> Result<(), anyhow::Error> {
        let task_context: &super::TaskContext = state.get()?;
        let docker: &Docker = state.get()?;

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

            run_command_builder.env(self.env);

            docker
                .run_command(
                    state,
                    run_command_builder
                        .build()
                        .map_err(|e| anyhow!("Invalid run commands params: {}", e))?,
                )
                .await?;
        } else {
            let tempdir = tempfile::TempFile::dir().await?;
            info!("Using context directory: {:?}", tempdir.path);

            for (link, path) in task_context.links.iter() {
                tokio::fs::symlink(path, tempdir.path.join(link)).await?;
            }

            let mut command = tokio::process::Command::new(interpreter);
            command.current_dir(&tempdir.path);
            command.args(args);
            command.arg(&script_file.path);

            run_command_with_log(state, command).await?;
        };

        Ok(())
    }
}

fn get_interpreter_args(
    interpreter: Option<Vec<String>>,
) -> Result<(String, Vec<String>), anyhow::Error> {
    if let Some(command) = interpreter {
        let mut it = command.into_iter();
        let interpreter = it
            .next()
            .ok_or_else(|| anyhow!("Intepreter is not specified"))?;
        let args = it.collect();

        Ok((interpreter, args))
    } else {
        Ok((
            String::from(DEFAULT_INTERPRETER),
            DEFAULT_ARGS.iter().map(|s| String::from(*s)).collect(),
        ))
    }
}

pub async fn run_command_with_log<'a>(
    state: &State<'a>,
    mut command: tokio::process::Command,
) -> Result<ExitStatus, anyhow::Error> {
    let mut logger = Logger::new(state).await?;

    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command.spawn()?;
    let stdout = LinesStream::new(BufReader::new(child.stdout.take().unwrap()).lines());
    let stderr = LinesStream::new(BufReader::new(child.stderr.take().unwrap()).lines());

    let mut child_out = stdout
        .map(OutputLine::Out)
        .merge(stderr.map(OutputLine::Err));

    while let Some(line) = child_out.next().await {
        let log_line = match line {
            OutputLine::Out(text) => logger.regular(text?).await?,
            OutputLine::Err(text) => logger.error(text?).await?,
        };
    }

    let status = child.wait().await?;

    info!("Script done with exit status {}", status);

    Ok(status)
}

enum OutputLine<T> {
    Out(T),
    Err(T),
}
