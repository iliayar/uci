use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_stream::{wrappers::LinesStream, StreamExt};

use log::*;

pub async fn run_command_with_output(
    mut command: tokio::process::Command,
) -> Result<(), tokio::io::Error> {
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let child = command.spawn()?;
    let stdout = LinesStream::new(BufReader::new(child.stdout.unwrap()).lines());
    let stderr = LinesStream::new(BufReader::new(child.stderr.unwrap()).lines());

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

    info!("Script done");

    Ok(())
}

enum OutputLine<T> {
    Out(T),
    Err(T),
}
