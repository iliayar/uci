use std::io::Write;

use crate::{execute, utils::Spinner};

use log::*;
use termion::{clear, color, scroll, style};

pub async fn execute_repo_update(
    config: &crate::config::Config,
    repo_id: String,
) -> Result<(), execute::ExecuteError> {
    let project_id = config.get_project();
    debug!("Executing action call command");

    let body = common::runner::UpdateRepoQuery {
        project_id,
        repo_id,
    };
    let response = crate::runner::post_body(config, "/update", &body)?
        .send()
        .await;
    let response: common::runner::ContinueReponse = crate::runner::json(response).await?;

    debug!("Will follow run {}", response.run_id);

    let mut ws_client = crate::runner::ws(config, response.run_id).await?;

    match ws_client
        .receive::<common::runner::UpdateRepoMessage>()
        .await
    {
        Some(common::runner::UpdateRepoMessage::PullingRepo) => {
            println!(
                "{}Pulling repo {bold}{}{no_bold} in project {bold}{}{no_bold} {}",
                color::Fg(color::Blue),
                body.repo_id,
                body.project_id,
                style::Reset,
                bold = style::Bold,
                no_bold = style::NoBold,
            );
        }
        Some(msg) => {
            return Err(execute::ExecuteError::Fatal(format!(
                "Unexpected message: {:?}",
                msg
            )));
        }
        None => {
            return Err(execute::ExecuteError::Fatal(
                "Expected a message".to_string(),
            ));
        }
    }

    let mut spinner = Spinner::new();
    loop {
        if let Some(message) = ws_client
            .try_receive::<common::runner::UpdateRepoMessage>()
            .await
        {
            match message {
                common::runner::UpdateRepoMessage::NoSuchRepo => {
                    println!(
                        "{}No such repo {bold}{}{no_bold} in project {bold}{}{no_bold} {}",
                        color::Fg(color::Red),
                        body.repo_id,
                        body.project_id,
                        style::Reset,
                        bold = style::Bold,
                        no_bold = style::NoBold,
                    );
                }
                common::runner::UpdateRepoMessage::RepoPulled { changed_files } => {
                    println!(
                        "{}{}Repo {}{}{} pulled{}",
                        clear::CurrentLine,
                        color::Fg(color::Green),
                        style::Bold,
                        body.repo_id,
                        style::NoBold,
                        style::Reset
                    );

                    if changed_files.is_empty() {
                        println!("No changes");
                    } else {
                        println!("{}Changed files{}:", style::Bold, style::Reset);
                        for file in changed_files.into_iter() {
                            println!("  {}{}{}", style::Italic, file, style::Reset);
                        }
                    }
                }
                common::runner::UpdateRepoMessage::FailedToPull { err } => {
                    println!(
                        "{} Failed to pull repo {}{}{}: {}{}",
                        color::Fg(color::Red),
                        style::Bold,
                        body.repo_id,
                        style::NoBold,
                        err,
                        style::Reset,
                    );
                }
                msg => {
                    return Err(execute::ExecuteError::Warning(format!(
                        "Unexpected message: {:?}",
                        msg
                    )));
                }
            }
            break;
        }

        println!(
            "[{}{}{}] Pulling repo {}",
            color::Fg(color::Blue),
            spinner.next(),
            style::Reset,
            body.repo_id
        );

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        print!("{}", scroll::Down(1));
        std::io::stdout()
            .flush()
            .map_err(|err| execute::ExecuteError::Fatal(err.to_string()))?;
    }

    execute::utils::print_clone_repos(&mut ws_client).await?;

    execute::utils::print_pipeline_run(&mut ws_client).await?;

    Ok(())
}
