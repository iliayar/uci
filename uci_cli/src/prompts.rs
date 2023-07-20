use crate::{config::Config, execute::ExecuteError};

use termion::{color, style};

use runner_client::*;

impl crate::select::SelectOption for models::Action {
    type Data = String;

    fn show(&self, out: &mut impl std::io::Write) {
        write!(out, "{}", self.id).ok();
    }

    fn data(self) -> Self::Data {
        self.id
    }

    fn data_name(&self) -> &str {
        "action"
    }
}

pub struct RunSelection {
    pub pipeline_id: String,
    pub run_id: String,
}

impl std::fmt::Display for RunSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at pipeline {}", self.run_id, self.pipeline_id)
    }
}

impl crate::select::SelectOption for models::Run {
    type Data = RunSelection;

    fn show(&self, out: &mut impl std::io::Write) {
        if let models::RunStatus::Running = self.status {
            write!(
                out,
                "{}In progress{} ",
                color::Fg(color::Green),
                style::Reset
            )
            .ok();
        }
        write!(out, "{} (pipeline: {})", self.run_id, self.pipeline).ok();
    }

    fn data(self) -> Self::Data {
        RunSelection {
            pipeline_id: self.pipeline,
            run_id: self.run_id,
        }
    }

    fn data_name(&self) -> &str {
        "run"
    }
}

pub async fn promp_action(config: &Config, project_id: String) -> Result<String, ExecuteError> {
    let actions = api::list_actions(config, project_id).await?;
    Ok(crate::select::prompt(actions.actions.into_iter())?)
}

impl crate::select::SelectOption for models::Repo {
    type Data = String;

    fn show(&self, out: &mut impl std::io::Write) {
        write!(out, "{}", self.id).ok();
    }

    fn data(self) -> Self::Data {
        self.id
    }

    fn data_name(&self) -> &str {
        "repo"
    }
}

pub async fn promp_repo(config: &Config, project_id: String) -> Result<String, ExecuteError> {
    let repos = api::repos_list(config, project_id).await?;
    Ok(crate::select::prompt(repos.repos.into_iter())?)
}

pub async fn promp_run(
    config: &Config,
    project_id: Option<String>,
    run_id: Option<String>,
    pipeline_id: Option<String>,
) -> Result<RunSelection, ExecuteError> {
    let runs = api::list_runs(config, project_id, pipeline_id).await?;
    let runs = if let Some(run_id) = run_id {
        runs.runs
            .into_iter()
            .filter(|r| r.run_id == run_id)
            .collect()
    } else {
        runs.runs
    };

    if runs.is_empty() {
        return Err(ExecuteError::Warning(format!("No any runs")));
    }

    Ok(crate::select::prompt(runs.into_iter())?)
}

impl crate::select::SelectOption for models::Service {
    type Data = String;

    fn show(&self, out: &mut impl std::io::Write) {
        match self.status {
            models::ServiceStatus::Running
            | models::ServiceStatus::Starting
            | models::ServiceStatus::Restarting => {
                write!(out, "{}Running{} ", color::Fg(color::Green), style::Reset).ok();
            }
            models::ServiceStatus::NotRunning
            | models::ServiceStatus::Dead
            | models::ServiceStatus::Exited(_) => {
                write!(out, "{}Not running{} ", color::Fg(color::Red), style::Reset).ok();
            }
            models::ServiceStatus::Unknown => {}
        }
        write!(out, "{}", self.id).ok();
    }

    fn data(self) -> Self::Data {
        self.id
    }

    fn data_name(&self) -> &str {
        "service"
    }
}

pub async fn promp_services(
    config: &Config,
    project_id: String,
) -> Result<Vec<String>, ExecuteError> {
    let services = api::list_services(config, project_id).await?;
    Ok(crate::select::prompt_many(services.services.into_iter())?)
}

impl crate::select::SelectOption for models::Project {
    type Data = String;

    fn show(&self, out: &mut impl std::io::Write) {
        write!(out, "{}", self.id).ok();
    }

    fn data(self) -> Self::Data {
        self.id
    }

    fn data_name(&self) -> &str {
        "project"
    }
}

pub async fn promp_project(config: &Config) -> Result<String, ExecuteError> {
    let projects = api::projects_list(config).await?;
    Ok(crate::select::prompt(projects.projects.into_iter())?)
}
