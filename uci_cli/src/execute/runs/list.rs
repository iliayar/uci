use crate::execute;

use termion::{color, style};

use log::*;

pub async fn execute_runs_list(
    config: &crate::config::Config,
    pipeline_id: Option<String>,
) -> Result<(), execute::ExecuteError> {
    let project_id = config.try_get_project().await;
    debug!("Executing runs list command");

    let response = crate::runner::api::list_runs(config, project_id, pipeline_id).await?;

    println!("{}Runs{}:", style::Bold, style::Reset);
    for run in response.runs.into_iter() {
        // FIXME: Print structured by projects, pipelines
        println!("- Project: {}", run.project);
        println!("  Pipeline: {}", run.pipeline);
        println!("  Run: {}", run.run_id);
        println!("  Started: {}", run.started);

        match run.status {
            common::runner::RunStatus::Running => {
                println!(
                    "  Status: {}Running{}",
                    color::Fg(color::Blue),
                    style::Reset
                )
            }
            common::runner::RunStatus::Finished(finished_status) => match finished_status {
                common::runner::RunFinishedStatus::Success => {
                    println!(
                        "  Status: {}Finished{}",
                        color::Fg(color::Green),
                        style::Reset
                    )
                }
                common::runner::RunFinishedStatus::Error { message } => {
                    println!(
                        "  Status: {}Finished ({}){}",
                        color::Fg(color::Green),
                        message,
                        style::Reset
                    )
                }
            },
        }

        println!("  Jobs:");
        for (job_id, job) in run.jobs.into_iter() {
            println!("  - Job: {}", job_id);

            match job.status {
                common::runner::JobStatus::Pending => {
                    println!(
                        "    Status: {}Pending{}",
                        color::Fg(color::LightBlack),
                        style::Reset
                    )
                }
                common::runner::JobStatus::Running { step } => {
                    println!(
                        "    Status: {}Running #{}{}",
                        color::Fg(color::Blue),
                        step,
                        style::Reset
                    )
                }
                common::runner::JobStatus::Finished { error } => {
                    if let Some(error) = error {
                        println!(
                            "    Status: {}Failed: {}{}",
                            color::Fg(color::Red),
                            error,
                            style::Reset
                        )
                    } else {
                        println!(
                            "    Status: {}Finished{}",
                            color::Fg(color::Green),
                            style::Reset
                        )
                    }
                }
            }
        }
    }

    Ok(())
}
