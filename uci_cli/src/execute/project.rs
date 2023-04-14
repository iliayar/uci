use crate::cli::*;

use log::*;
use termion::{color, style};

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
    let response: common::runner::EmptyResponse = crate::runner::json(response).await?;

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

    println!(
        "{}Pulling repo {}{}{} in project {}{}{} {}",
        color::Fg(color::Blue),
        style::Bold,
        repo_id,
        style::NoBold,
        style::Bold,
        project_id,
        style::NoBold,
        style::Reset
    );

    debug!("Will follow with client_id {}", response.client_id);
    todo!();

    Ok(())
}
