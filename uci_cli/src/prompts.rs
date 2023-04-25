use crate::{config::Config, execute::ExecuteError};

use termion::{color, style};

impl crate::select::SelectOption for common::runner::Action {
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

impl crate::select::SelectOption for common::runner::Run {
    type Data = RunSelection;

    fn show(&self, out: &mut impl std::io::Write) {
        if let common::runner::RunStatus::Running = self.status {
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
    let actions = crate::runner::api::actions_list(config, project_id).await?;
    Ok(crate::select::prompt(actions.actions.into_iter())?)
}

pub async fn promp_run(
    config: &Config,
    project_id: Option<String>,
    run_id: Option<String>,
    pipeline_id: Option<String>,
) -> Result<RunSelection, ExecuteError> {
    let runs = crate::runner::api::list_runs(config, project_id, pipeline_id).await?;
    let runs = if let Some(run_id) = run_id {
        runs.runs
            .into_iter()
            .filter(|r| r.run_id == run_id)
            .collect()
    } else {
        runs.runs
    };

    Ok(crate::select::prompt(runs.into_iter())?)
}

impl crate::select::SelectOption for common::runner::Service {
    type Data = String;

    fn show(&self, out: &mut impl std::io::Write) {
        match self.status {
            common::runner::ServiceStatus::Running
            | common::runner::ServiceStatus::Starting
            | common::runner::ServiceStatus::Restarting => {
                write!(out, "{}Running{} ", color::Fg(color::Green), style::Reset).ok();
            }
            common::runner::ServiceStatus::NotRunning
            | common::runner::ServiceStatus::Dead
            | common::runner::ServiceStatus::Exited => {
                write!(out, "{}Not running{} ", color::Fg(color::Red), style::Reset).ok();
            }
            common::runner::ServiceStatus::Unknown => {}
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
    let services = crate::runner::api::list_services(config, project_id).await?;
    Ok(crate::select::prompt_many(services.services.into_iter())?)
}
