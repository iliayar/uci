use std::io::Write;

use crate::{cli::*, utils::Spinner};

use log::*;
use termion::{color, scroll, style, clear};

pub async fn execute_project(
    config: &crate::config::Config,
    command: ProjectCommands,
) -> Result<(), super::ExecuteError> {
    match command {
        ProjectCommands::List {} => execute_project_list(config).await?,
        ProjectCommands::Actions { command } => execute_trigger(config, command).await?,
        ProjectCommands::Repos { command } => execute_repo(config, command).await?,
    }

    Ok(())
}

pub async fn execute_project_list(
    config: &crate::config::Config,
) -> Result<(), super::ExecuteError> {
    debug!("Executing project list command");

    let response = crate::runner::get(config, "/projects/list")?.send().await;
    let response: common::runner::ProjectsListResponse = crate::runner::json(response).await?;

    println!("{}Projects{}:", style::Bold, style::Reset);
    for project_id in response.projects.into_iter() {
        println!("- {}", project_id);
    }

    Ok(())
}

pub async fn execute_trigger(
    config: &crate::config::Config,
    command: ActionCommand,
) -> Result<(), super::ExecuteError> {
    match command {
        ActionCommand::Call {
            project_id,
            action_id,
        } => execute_trigger_call(config, project_id, action_id).await?,
    }

    Ok(())
}

pub async fn execute_trigger_call(
    config: &crate::config::Config,
    project_id: String,
    action_id: String,
) -> Result<(), super::ExecuteError> {
    debug!("Executing action call command");

    let response = crate::runner::post(config, format!("/call/{}/{}", project_id, action_id))?
        .send()
        .await;
    let response: common::runner::ContinueReponse = crate::runner::json(response).await?;

    debug!("Will follow run {}", response.run_id);
    let mut ws_client = crate::runner::ws(config, response.run_id).await?;

    println!(
        "{}Triggered action {}{}{} on project {}{}{} {}",
        color::Fg(color::Green),
        style::Bold,
        action_id,
        style::NoBold,
        style::Bold,
        project_id,
        style::NoBold,
        style::Reset
    );

    super::utils::print_clone_repos(&mut ws_client).await?;

    Ok(())
}

pub async fn execute_repo(
    config: &crate::config::Config,
    command: RepoCommand,
) -> Result<(), super::ExecuteError> {
    match command {
        RepoCommand::Update {
            project_id,
            repo_id,
        } => execute_repo_update(config, project_id, repo_id).await?,
    }

    Ok(())
}

pub async fn execute_repo_update(
    config: &crate::config::Config,
    project_id: String,
    repo_id: String,
) -> Result<(), super::ExecuteError> {
    debug!("Executing action call command");

    let response = crate::runner::post(config, format!("/update/{}/{}", project_id, repo_id))?
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
                repo_id,
                project_id,
                style::Reset,
                bold = style::Bold,
                no_bold = style::NoBold,
            );
        }
        Some(msg) => {
            return Err(super::ExecuteError::Fatal(format!(
                "Unexpected message: {:?}",
                msg
            )));
        }
        None => {
            return Err(super::ExecuteError::Fatal("Expected a message".to_string()));
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
                        repo_id,
                        project_id,
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
                        repo_id,
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
                        repo_id,
                        style::NoBold,
                        err,
                        style::Reset,
                    );
                }
                msg => {
                    return Err(super::ExecuteError::Warning(format!(
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
            repo_id
        );

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        print!("{}", scroll::Down(1));
        std::io::stdout()
            .flush()
            .map_err(|err| super::ExecuteError::Fatal(err.to_string()))?;
    }

    super::utils::print_clone_repos(&mut ws_client).await?;

    Ok(())
}
