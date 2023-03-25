use std::collections::HashMap;

use crate::lib::{
    docker,
    utils::{
        shell::{self, run_command_with_output},
        tempfile,
    },
};

use super::task;

use anyhow::anyhow;

const DEFAULT_INTERPRETER: &str = "/usr/bin/env";
const DEFAULT_ARGS: [&str; 1] = ["bash"];

#[async_trait::async_trait]
impl task::Task for common::RunShellConfig {
    async fn run(self, context: &crate::lib::context::Context) -> Result<(), task::TaskError> {
        let (interpreter, mut args) = get_interpreter_args(self.interpreter)?;
        let script_file = tempfile::TempFile::new_executable(&self.script).await?;

        if let Some(image) = self.docker_image {
            let container_script_file = String::from("/script");

            let mut run_command_builder = docker::RunCommandParamsBuilder::default();
            run_command_builder.image(image);

            let mut command = vec![interpreter];
            command.append(&mut args);
            command.push(container_script_file.clone());
            run_command_builder.command(command);

            let mut mounts = HashMap::new();
            mounts.insert(script_file.path.clone(), container_script_file.clone());
            run_command_builder.mounts(mounts);

            context
                .docker()
                .run_command(
                    run_command_builder
                        .build()
                        .map_err(|e| anyhow!("Invalid run commands params: {}", e))?,
                )
                .await?;
        } else {
            let mut command = tokio::process::Command::new(interpreter);
            command.args(args);
            command.arg(&script_file.path);

            shell::run_command_with_output(command).await?;
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
