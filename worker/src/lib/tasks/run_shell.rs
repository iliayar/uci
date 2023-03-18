use std::io::{Stderr, Stdout};
use std::process::Stdio;
use std::time::Duration;
use std::{fs::Permissions, os::unix::prelude::PermissionsExt};

use super::error::TaskError;
use crate::lib::docker::Docker;
use crate::lib::utils::tempfile::TempFile;
use bollard::container::{AttachContainerOptions, Config, CreateContainerOptions, LogOutput};
use common::RunShellConfig;
use log::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio_stream::wrappers::LinesStream;
use tokio_stream::StreamExt;

pub async fn run_shell_command(docker: &Docker, config: RunShellConfig) -> Result<(), TaskError> {
    if let Some(image) = config.docker_image {
        info!("Executing script in cotainer of image {}", image);
        run_shell_command_docker(docker, config.script, image).await?;
    } else {
        info!("Executing native script");
        run_shell_command_native(config.script).await?;
    }

    Ok(())
}

pub async fn run_shell_command_native(script: String) -> Result<(), TaskError> {
    // TODO: Get interpreter from config
    let mut command = tokio::process::Command::new("bash");
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.stdin(Stdio::piped());

    info!("Spawning process with native script",);

    let child = command.spawn()?;
    let stdout = LinesStream::new(BufReader::new(child.stdout.unwrap()).lines());
    let stderr = LinesStream::new(BufReader::new(child.stderr.unwrap()).lines());
    let mut stdin = child.stdin.unwrap();
    stdin.write_all(script.as_bytes()).await?;
    stdin.write_all("\n".as_bytes()).await?; // FIXME?: Execute last command

    let mut child_out = stdout
        .map(|e| OutputLine::Out(e))
        .merge(stderr.map(|e| OutputLine::Err(e)));

    while let Some(line) = child_out.next().await {
        match line {
            OutputLine::Out(line) => {
                info!("Script out: {}", line?);
            }
            OutputLine::Err(line) => {
                error!("Script err: {}", line?);
            }
        }
    }

    info!("Script done");

    Ok(())
}

pub async fn run_shell_command_docker(
    docker: &Docker,
    script: String,
    image: String,
) -> Result<(), TaskError> {
    // FIXME: Copied from docker_run

    let name = docker
        .con
        .create_container::<&str, &str>(
            Some(CreateContainerOptions {
                ..Default::default()
            }),
            Config {
                image: Some(&image),
                cmd: Some(vec!["bash"]),
		open_stdin: Some(true),
                ..Default::default()
            },
        )
        .await?
        .id;
    info!("Created container '{}'", name);

    docker.con.start_container::<&str>(&name, None).await?;
    info!("Container started '{}'", name);

    let mut attach = docker
        .con
        .attach_container::<&str>(
            &name,
            Some(AttachContainerOptions {
                stdin: Some(true),
                stdout: Some(true),
                stderr: Some(true),
		stream: Some(true),
		detach_keys: Some("ctrl-c"),
                ..Default::default()
            }),
        )
        .await?;

    info!("Seding script to container '{}'", name);
    attach.input.write_all(script.as_bytes()).await?;
    attach.input.write_all(b"\nexit\n").await?; // FIXME: Do not use exit

    info!("Script was sent to container '{}'", name);
    while let Some(line) = attach.output.next().await {
        match line? {
            LogOutput::StdErr { message } => {
                error!(
                    "Conainer \"{}\" err: {}",
                    name,
                    String::from_utf8_lossy(message.as_ref())
                );
            }
            LogOutput::StdOut { message } => {
                info!(
                    "Conainer \"{}\" out: {}",
                    name,
                    String::from_utf8_lossy(message.as_ref())
                );
	    },
            LogOutput::StdIn { message } => {
		warn!("Docker script should not produce stdin messages");
	    },
            LogOutput::Console { message } => {
		warn!("Docker script should not produce console messages");
	    },
        }
    }

    info!("Script in container {} done", name);

    docker.con.stop_container(&name, None).await?;
    docker.con.remove_container(&name, None).await?;

    Ok(())
}

enum OutputLine<T> {
    Out(T),
    Err(T),
}
