use std::process::{ExitStatus, Stdio};

use tokio::io::{AsyncBufReadExt, BufReader};
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
        .map(OutputLine::Out)
        .merge(stderr.map(OutputLine::Err));

    while let Some(line) = child_out.next().await {
        match line {
            OutputLine::Out(line) => {
                trace!("stdout> {}", line?);
            }
            OutputLine::Err(line) => {
                trace!("stderr> {}", line?);
            }
        }
    }

    let status = child.wait().await?;

    debug!("Script done with exit status {}", status);

    Ok(status)
}

enum OutputLine<T> {
    Out(T),
    Err(T),
}
