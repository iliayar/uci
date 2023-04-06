use std::process::{ExitStatus, Stdio};

use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio_stream::{wrappers::LinesStream, StreamExt};

use log::*;

pub async fn run_command_with_output(
    mut command: tokio::process::Command,
) -> Result<ExitStatus, tokio::io::Error> {
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command.spawn()?;
    let stdout = LinesStream::new(BufReader::new(child.stdout.take().unwrap()).lines());
    let stderr = LinesStream::new(BufReader::new(child.stderr.take().unwrap()).lines());

    let mut child_out = stdout
        .map(|e| OutputLine::Out(e))
        .merge(stderr.map(|e| OutputLine::Err(e)));

    while let Some(line) = child_out.next().await {
        match line {
            OutputLine::Out(line) => {
                info!("{}", line?);
            }
            OutputLine::Err(line) => {
                error!("{}", line?);
            }
        }
    }

    let status = child.wait().await?;

    info!("Script done with exit status {}", status);

    Ok(status)
}

enum OutputLine<T> {
    Out(T),
    Err(T),
}