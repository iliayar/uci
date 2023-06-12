use crate::execute;

use termion::{color, style};

use log::*;

use runner_client::*;

pub async fn execute_runs_list(
    config: &crate::config::Config,
    pipeline_id: Option<String>,
) -> Result<(), execute::ExecuteError> {
    let project_id = config.try_get_project().await;
    debug!("Executing runs list command");

    let response = api::list_runs(config, project_id, pipeline_id).await?;

    println!("{}Runs{}:", style::Bold, style::Reset);
    for run in response.runs.into_iter() {
        // FIXME: Print structured by projects, pipelines
        println!("- Project: {}", run.project);
        println!("  Pipeline: {}", run.pipeline);
        println!("  Run: {}", run.run_id);
        println!("  Started: {}", run.started);

        match run.status {
            models::RunStatus::Running => {
                println!(
                    "  Status: {}Running{}",
                    color::Fg(color::Blue),
                    style::Reset
                )
            }
            models::RunStatus::Finished(finished_status) => match finished_status {
                models::RunFinishedStatus::Success => {
                    println!(
                        "  Status: {}Finished{}",
                        color::Fg(color::Green),
                        style::Reset
                    )
                }
                models::RunFinishedStatus::Canceled => {
                    println!(
                        "  Status: {}Canceled{}",
                        color::Fg(color::Yellow),
                        style::Reset
                    )
                }
                models::RunFinishedStatus::Displaced => {
                    println!(
                        "  Status: {}Displaced{}",
                        color::Fg(color::LightBlack),
                        style::Reset
                    )
                }
                models::RunFinishedStatus::Error { message } => {
                    println!(
                        "  Status: {}Finished ({}){}",
                        color::Fg(color::Red),
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
                models::JobStatus::Canceled => {
                    println!(
                        "    Status: {}Canceled{}",
                        color::Fg(color::Yellow),
                        style::Reset
                    )
                }
                models::JobStatus::Skipped => {
                    println!(
                        "    Status: {}Skipped{}",
                        color::Fg(color::LightBlack),
                        style::Reset
                    )
                }
                models::JobStatus::Pending => {
                    println!(
                        "    Status: {}Pending{}",
                        color::Fg(color::LightBlack),
                        style::Reset
                    )
                }
                models::JobStatus::Running { step } => {
                    println!(
                        "    Status: {}Running #{}{}",
                        color::Fg(color::Blue),
                        step,
                        style::Reset
                    )
                }
                models::JobStatus::Finished { error } => {
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
