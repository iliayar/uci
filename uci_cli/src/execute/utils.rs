use std::io::Write;
use std::path::PathBuf;
use std::{collections::HashMap, sync::Arc};

use crate::config::Config;
use crate::utils::{ucolor, WithSpinner};
use crate::{runner::WsClient, utils::Spinner};

use termion::{clear, color, cursor, raw::IntoRawMode, style};
use tokio::sync::Mutex;

use log::*;

pub async fn upload_archive(
    config: &Config,
    dirpath: PathBuf,
) -> Result<String, super::ExecuteError> {
    debug!("Executing upload");

    // NOTE: Maybe it's not good to store archive in memory
    let mut tar_builder = tokio_tar::Builder::new(Vec::new());

    tar_builder
        .append_dir_all(".", dirpath)
        .with_spinner("Building tar")
        .await
        .map_err(|err| {
            super::ExecuteError::Fatal(format!("Failed to create archive with repo: {}", err))
        })?;

    let data = tar_builder
        .into_inner()
        .with_spinner("Building tar")
        .await
        .map_err(|err| {
            super::ExecuteError::Fatal(format!("Failed to create archive with repo: {}", err))
        })?;
    let response = crate::runner::api::upload(config, data)
        .with_spinner("Uploading tar")
        .await?;

    println!(
        "{}Uploaded artifact: {}{}",
        color::Fg(color::Green),
        response.artifact,
        style::Reset
    );

    Ok(response.artifact)
}

pub async fn print_clone_repos(ws_client: &mut WsClient) -> Result<(), super::ExecuteError> {
    match ws_client
        .receive::<models::CloneMissingRepos>()
        .await
    {
        Some(models::CloneMissingRepos::Begin) => {}
        _ => {
            return Err(super::ExecuteError::Warning(
                "Expect begin message for clone missing repos".to_string(),
            ));
        }
    }

    enum Status {
        InProgress,
        Done,
    }

    let mut repos_to_clone: HashMap<String, Status> = HashMap::new();
    let mut spinner = Spinner::new();

    loop {
        if let Some(message) = ws_client
            .try_receive::<models::CloneMissingRepos>()
            .await
        {
            match message {
                models::CloneMissingRepos::Begin => unreachable!(),
                models::CloneMissingRepos::ClonningRepo { repo_id } => {
                    if repos_to_clone.is_empty() {
                        println!(
                            "{}Clonning missing repos:{}",
                            color::Fg(color::Blue),
                            style::Reset
                        );
                    }
                    repos_to_clone.insert(repo_id, Status::InProgress);
                }
                models::CloneMissingRepos::RepoCloned { repo_id } => {
                    repos_to_clone.insert(repo_id, Status::Done);
                }
                models::CloneMissingRepos::Finish => {
                    if !repos_to_clone.is_empty() {
                        let mut stdout = std::io::stdout().into_raw_mode().unwrap();
                        print!("{}{}", cursor::Up(1), clear::AfterCursor);
                        stdout.flush().ok();
                        drop(stdout);

                        println!(
                            "{}Missing repos cloned{}",
                            color::Fg(color::Green),
                            style::Reset
                        );
                    }
                    break;
                }
            }
        }

        let ch = spinner.next();
        for (repo, status) in repos_to_clone.iter() {
            match status {
                Status::InProgress => {
                    println!(
                        "  [{}{}{}] {}",
                        color::Fg(color::Blue),
                        ch,
                        style::Reset,
                        repo
                    );
                }
                Status::Done => {
                    println!(
                        "  [{}DONE{}] {}",
                        color::Fg(color::Green),
                        style::Reset,
                        repo
                    );
                }
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        if !repos_to_clone.is_empty() {
            let mut stdout = std::io::stdout().into_raw_mode().unwrap();
            write!(
                stdout,
                "{}{}",
                cursor::Up(repos_to_clone.len() as u16),
                clear::AfterCursor
            )
            .ok();
            stdout.flush().ok();
        }
    }

    Ok(())
}

struct RunState {
    pipelines: HashMap<String, PipelineState>,
    prev_lines: usize,
    spinner: Spinner,
    print_state: bool,
}

struct PipelineState {
    status: PipelineStatus,
    jobs: HashMap<String, JobStatus>,
}

enum PipelineStatus {
    Running,
    Finished,
    FinishedError { message: String },
    Canceled,
    Displaced,
}

enum JobStatus {
    Pending,
    Running { step: usize },
    Finished { error: Option<String> },
    Skipped,
    Canceled,
}

impl Default for RunState {
    fn default() -> Self {
        RunState::new(true)
    }
}

impl RunState {
    fn from_runs_list(
        print_state: bool,
        run_id: String,
        runs_list: models::ListRunsResponse,
    ) -> Self {
        let mut state = RunState::new(print_state);

        for run in runs_list.runs.into_iter() {
            if run.run_id == run_id {
                for (job_id, job) in run.jobs {
                    let job_status = match job.status {
                        models::JobStatus::Canceled => JobStatus::Canceled,
                        models::JobStatus::Skipped => JobStatus::Skipped,
                        models::JobStatus::Pending => JobStatus::Pending,
                        models::JobStatus::Running { step } => JobStatus::Running { step },
                        models::JobStatus::Finished { error } => {
                            JobStatus::Finished { error }
                        }
                    };

                    state.set_job_status(run.pipeline.clone(), job_id, job_status);
                }

                if let models::RunStatus::Finished(finished_status) = run.status {
                    let status = match finished_status {
                        models::RunFinishedStatus::Error { message } => {
                            PipelineStatus::FinishedError { message }
                        }
                        models::RunFinishedStatus::Success => PipelineStatus::Finished,
                        models::RunFinishedStatus::Canceled => PipelineStatus::Canceled,
                        models::RunFinishedStatus::Displaced => PipelineStatus::Displaced,
                    };

                    state.finish_pipeline(run.pipeline, status);
                }
            }
        }

        state
    }

    fn new(print_state: bool) -> Self {
        Self {
            pipelines: HashMap::new(),
            prev_lines: 0,
            spinner: Spinner::new(),
            print_state,
        }
    }
    fn print(&mut self) -> Result<(), super::ExecuteError> {
        if !self.print_state {
            return Ok(());
        }

        self.clear()?;
        let mut lines = 0usize;

        println!("--------------");
        lines += 1;

        for (pipeline_id, pipeline) in self.pipelines.iter() {
            match &pipeline.status {
                PipelineStatus::Running => {
                    print!("{}Running{}", color::Fg(color::Blue), style::Reset)
                }
                PipelineStatus::Finished => {
                    print!("{}Finished{}", color::Fg(color::Green), style::Reset)
                }
                PipelineStatus::Canceled => {
                    print!("{}Canceled{}", color::Fg(color::Yellow), style::Reset)
                }
                PipelineStatus::Displaced => {
                    print!("{}Displaced{}", color::Fg(color::LightBlack), style::Reset)
                }
                PipelineStatus::FinishedError { message } => {
                    print!("{}Failed{}", color::Fg(color::Red), style::Reset)
                }
            }

            print!(" {}", pipeline_id);

            match &pipeline.status {
                PipelineStatus::Running
                | PipelineStatus::Finished
                | PipelineStatus::Canceled
                | PipelineStatus::Displaced => println!(),
                PipelineStatus::FinishedError { message } => {
                    println!(" {}{}{}", color::Fg(color::Red), message, style::Reset)
                }
            }

            lines += 1;

            for (job_id, job_status) in pipeline.jobs.iter() {
                print!("  ");
                match job_status {
                    JobStatus::Canceled => {
                        print!("{}Canceled{}", color::Fg(color::Yellow), style::Reset)
                    }
                    JobStatus::Skipped => {
                        print!("{}Skipped{}", color::Fg(color::LightBlack), style::Reset)
                    }
                    JobStatus::Pending => print!("Pending"),
                    JobStatus::Running { step } => print!(
                        "[{}{}{}] #{}",
                        color::Fg(color::Blue),
                        self.spinner.peek(),
                        style::Reset,
                        step
                    ),
                    JobStatus::Finished { error } => {
                        if let Some(error) = error {
                            print!("{}Failed: {}{}", color::Fg(color::Red), error, style::Reset)
                        } else {
                            print!("{}Finished{}", color::Fg(color::Green), style::Reset)
                        }
                    }
                }

                println!(" {}", job_id);
                lines += 1;
            }
        }
        self.prev_lines = lines;
        Ok(())
    }

    fn clear(&mut self) -> Result<(), super::ExecuteError> {
        if self.prev_lines == 0 {
            return Ok(());
        }

        let mut stdout = std::io::stdout().into_raw_mode().unwrap();
        write!(
            stdout,
            "{}{}",
            cursor::Up(self.prev_lines as u16),
            clear::AfterCursor
        )
        .ok();
        stdout.flush().ok();
        self.prev_lines = 0;
        Ok(())
    }

    fn start_pipeline(&mut self, pipeline: String) {
        self.pipelines.insert(
            pipeline,
            PipelineState {
                status: PipelineStatus::Running,
                jobs: HashMap::new(),
            },
        );
    }

    fn finish_pipeline(&mut self, pipeline: String, status: PipelineStatus) {
        if let Some(pipeline) = self.pipelines.get_mut(&pipeline) {
            pipeline.status = status;
        } else {
            self.pipelines.insert(
                pipeline,
                PipelineState {
                    status,
                    jobs: HashMap::new(),
                },
            );
        }
    }

    fn set_job_status(&mut self, pipeline: String, job: String, status: JobStatus) {
        if !self.pipelines.contains_key(&pipeline) {
            self.start_pipeline(pipeline.to_string());
        }

        let pipeline = self.pipelines.get_mut(&pipeline).unwrap();
        pipeline.jobs.insert(job, status);
    }
}

pub async fn print_pipeline_run(ws_client: &mut WsClient) -> Result<(), super::ExecuteError> {
    let state = Arc::new(Mutex::new(RunState::new(true)));
    print_pipeline_run_impl(ws_client, state, None).await?;
    Ok(())
}

pub async fn print_pipeline_run_init(
    ws_client: &mut WsClient,
    run_id: String,
    runs_list: models::ListRunsResponse,
) -> Result<(), super::ExecuteError> {
    let state = Arc::new(Mutex::new(get_state_with_init(
        true,
        Some((run_id, runs_list)),
    )));
    print_pipeline_run_impl(ws_client, state, None).await?;
    Ok(())
}

pub async fn print_pipeline_run_follow(
    ws_client: &mut WsClient,
    follow_ws_client: &mut WsClient,
    init_state: models::ListRunsResponse,
) -> Result<(), super::ExecuteError> {
    print_pipeline_run_follow_impl(ws_client, follow_ws_client, true, None).await
}

pub async fn print_pipeline_run_follow_init(
    ws_client: &mut WsClient,
    follow_ws_client: &mut WsClient,
    run_id: String,
    runs_list: models::ListRunsResponse,
) -> Result<(), super::ExecuteError> {
    print_pipeline_run_follow_impl(ws_client, follow_ws_client, true, Some((run_id, runs_list)))
        .await
}

pub async fn print_pipeline_run_no_state(
    ws_client: &mut WsClient,
) -> Result<(), super::ExecuteError> {
    let state = Arc::new(Mutex::new(get_state_with_init(false, None)));
    print_pipeline_run_impl(ws_client, state, None).await?;
    Ok(())
}

pub async fn print_pipeline_run_no_state_init(
    ws_client: &mut WsClient,
    run_id: String,
    runs_list: models::ListRunsResponse,
) -> Result<(), super::ExecuteError> {
    let state = Arc::new(Mutex::new(get_state_with_init(
        false,
        Some((run_id, runs_list)),
    )));
    print_pipeline_run_impl(ws_client, state, None).await?;
    Ok(())
}

pub async fn print_pipeline_run_no_state_follow(
    ws_client: &mut WsClient,
    follow_ws_client: &mut WsClient,
) -> Result<(), super::ExecuteError> {
    print_pipeline_run_follow_impl(ws_client, follow_ws_client, false, None).await
}

pub async fn print_pipeline_run_no_state_follow_init(
    ws_client: &mut WsClient,
    follow_ws_client: &mut WsClient,
    run_id: String,
    runs_list: models::ListRunsResponse,
) -> Result<(), super::ExecuteError> {
    print_pipeline_run_follow_impl(
        ws_client,
        follow_ws_client,
        false,
        Some((run_id, runs_list)),
    )
    .await
}

// FIXME: Prints mess
pub async fn print_pipeline_run_follow_impl(
    ws_client: &mut WsClient,
    follow_ws_client: &mut WsClient,
    print_state: bool,
    init_state: Option<(String, models::ListRunsResponse)>,
) -> Result<(), super::ExecuteError> {
    let state = Arc::new(Mutex::new(get_state_with_init(print_state, init_state)));
    let last_log = print_pipeline_run_impl(ws_client, state.clone(), None).await?;
    print_pipeline_run_impl(follow_ws_client, state, last_log).await?;
    Ok(())
}

fn get_state_with_init(
    print_state: bool,
    init_state: Option<(String, models::ListRunsResponse)>,
) -> RunState {
    if let Some((run_id, runs_list)) = init_state {
        RunState::from_runs_list(print_state, run_id, runs_list)
    } else {
        RunState::new(print_state)
    }
}

async fn print_pipeline_run_impl(
    ws_client: &mut WsClient,
    state: Arc<Mutex<RunState>>,
    since: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, super::ExecuteError> {
    debug!("Running print_pipeline_run_impl with since ({:?})", since);
    let mut last_log: Option<chrono::DateTime<chrono::Utc>> = None;

    let spinner_state = state.clone();
    let spinner_update = tokio::spawn(async move {
        loop {
            {
                let mut run_state = spinner_state.lock().await;
                run_state.spinner.next();
                run_state.print().ok();
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    });

    while let Some(message) = ws_client.receive::<models::PipelineMessage>().await {
        state.lock().await.clear()?;
        match message {
            models::PipelineMessage::Start { pipeline } => {
                state.lock().await.start_pipeline(pipeline);
            }
            models::PipelineMessage::Canceled { pipeline } => {
                state
                    .lock()
                    .await
                    .finish_pipeline(pipeline, PipelineStatus::Canceled);
            }
            models::PipelineMessage::Displaced { pipeline } => {
                state
                    .lock()
                    .await
                    .finish_pipeline(pipeline, PipelineStatus::Displaced);
            }
            models::PipelineMessage::Finish { pipeline, error } => {
                let status = if let Some(message) = error {
                    PipelineStatus::FinishedError { message }
                } else {
                    PipelineStatus::Finished
                };
                state.lock().await.finish_pipeline(pipeline, status);
            }
            models::PipelineMessage::JobPending { pipeline, job_id } => {
                state
                    .lock()
                    .await
                    .set_job_status(pipeline, job_id, JobStatus::Pending);
            }
            models::PipelineMessage::JobProgress {
                pipeline,
                job_id,
                step,
            } => {
                state
                    .lock()
                    .await
                    .set_job_status(pipeline, job_id, JobStatus::Running { step });
            }
            models::PipelineMessage::JobFinished {
                pipeline,
                job_id,
                error,
            } => {
                state
                    .lock()
                    .await
                    .set_job_status(pipeline, job_id, JobStatus::Finished { error });
            }
            models::PipelineMessage::JobSkipped { pipeline, job_id } => {
                state
                    .lock()
                    .await
                    .set_job_status(pipeline, job_id, JobStatus::Skipped);
            }
            models::PipelineMessage::JobCanceled { pipeline, job_id } => {
                state
                    .lock()
                    .await
                    .set_job_status(pipeline, job_id, JobStatus::Canceled);
            }
            models::PipelineMessage::Log {
                pipeline,
                job_id,
                t,
                text,
                timestamp,
            } => {
                if let Some(last_log_ts) = last_log.take() {
                    last_log = Some(last_log_ts.max(timestamp));
                } else {
                    last_log = Some(timestamp)
                };

                if let Some(since) = since.as_ref() {
                    if &timestamp <= since {
                        continue;
                    }
                }

                let _state_lock = state.lock().await;
                print!(
                    "{} [{}{} -> {}{}] ",
                    timestamp,
                    ucolor(&(&pipeline, &job_id)),
                    pipeline,
                    job_id,
                    style::Reset
                );

                match t {
                    models::LogType::Regular => println!("{}", text.trim_end()),
                    models::LogType::Error => {
                        println!(
                            "{}{}{}",
                            color::Fg(color::Red),
                            text.trim_end(),
                            style::Reset
                        )
                    }
                    models::LogType::Warning => {
                        println!(
                            "{}{}{}",
                            color::Fg(color::Yellow),
                            text.trim_end(),
                            style::Reset
                        )
                    }
                }
            }
            _ => return Err(super::ExecuteError::unexpected_message()),
        }

        state.lock().await.print()?;
    }

    spinner_update.abort();

    Ok(last_log)
}
