use std::io::Write;
use std::{collections::HashMap, sync::Arc};

use crate::utils::ucolor;
use crate::{runner::WsClient, utils::Spinner};

use termion::{clear, color, scroll, style};
use tokio::sync::Mutex;

pub async fn print_clone_repos(ws_client: &mut WsClient) -> Result<(), super::ExecuteError> {
    match ws_client
        .receive::<common::runner::CloneMissingRepos>()
        .await
    {
        Some(common::runner::CloneMissingRepos::Begin) => {}
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
            .try_receive::<common::runner::CloneMissingRepos>()
            .await
        {
            match message {
                common::runner::CloneMissingRepos::Begin => unreachable!(),
                common::runner::CloneMissingRepos::ClonningRepo { repo_id } => {
                    if repos_to_clone.is_empty() {
                        println!(
                            "{}Clonning missing repos:{}",
                            color::Fg(color::Blue),
                            style::Reset
                        );
                    }
                    repos_to_clone.insert(repo_id, Status::InProgress);
                }
                common::runner::CloneMissingRepos::RepoCloned { repo_id } => {
                    repos_to_clone.insert(repo_id, Status::Done);
                }
                common::runner::CloneMissingRepos::Finish => {
                    if !repos_to_clone.is_empty() {
                        print!("{}{}", scroll::Down(1), clear::CurrentLine);
                        std::io::stdout()
                            .flush()
                            .map_err(|err| super::ExecuteError::Fatal(err.to_string()))?;
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
            print!("{}", scroll::Down(repos_to_clone.len() as u16));
            std::io::stdout()
                .flush()
                .map_err(|err| super::ExecuteError::Fatal(err.to_string()))?;
        }
    }

    Ok(())
}

struct RunState {
    pipelines: HashMap<String, PipelineState>,
    prev_lines: usize,
    spinner: Spinner,
    spinner_char: char,
}

struct PipelineState {
    status: PipelineStatus,
    jobs: HashMap<String, JobStatus>,
}

enum PipelineStatus {
    Running,
    Finished,
    FinishedError { message: String },
}

enum JobStatus {
    Pending,
    Running { step: usize },
    Finished,
}

impl RunState {
    fn new() -> Self {
        let mut spinner = Spinner::new();
        Self {
            pipelines: HashMap::new(),
            prev_lines: 0,
            spinner_char: spinner.next(),
            spinner,
        }
    }
    fn print(&mut self) -> Result<(), super::ExecuteError> {
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
                PipelineStatus::FinishedError { message } => {
                    print!("{}Failed{}", color::Fg(color::Red), style::Reset)
                }
            }

            print!(" {}", pipeline_id);

            match &pipeline.status {
                PipelineStatus::Running | PipelineStatus::Finished => println!(),
                PipelineStatus::FinishedError { message } => {
                    println!(" {}{}{}", color::Fg(color::Red), message, style::Reset)
                }
            }

            lines += 1;

            for (job_id, job_status) in pipeline.jobs.iter() {
                print!("  ");
                match job_status {
                    JobStatus::Pending => print!("Pending"),
                    JobStatus::Running { step } => print!(
                        "[{}{}{}] #{}",
                        color::Fg(color::Blue),
                        self.spinner_char,
                        style::Reset,
                        step
                    ),
                    JobStatus::Finished => {
                        print!("{}Finished{}", color::Fg(color::Green), style::Reset)
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

        print!(
            "{}{}",
            scroll::Down(self.prev_lines as u16),
            clear::CurrentLine
        );
        std::io::stdout()
            .flush()
            .map_err(|err| super::ExecuteError::Fatal(err.to_string()))?;
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

    fn finish_pipeline(&mut self, pipeline: String, error: Option<String>) {
        let status = if let Some(message) = error {
            PipelineStatus::FinishedError { message }
        } else {
            PipelineStatus::Finished
        };

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
        pipeline.jobs.insert(job.to_string(), status);
    }
}

pub async fn print_pipeline_run(ws_client: &mut WsClient) -> Result<(), super::ExecuteError> {
    let state = Arc::new(Mutex::new(RunState::new()));

    let spinner_state = state.clone();
    let spinner_update = tokio::spawn(async move {
        loop {
            {
                let mut run_state = spinner_state.lock().await;
                run_state.spinner_char = run_state.spinner.next();
                run_state.print().ok();
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    });

    while let Some(message) = ws_client.receive::<common::runner::PipelineMessage>().await {
        state.lock().await.clear()?;
        match message {
            common::runner::PipelineMessage::Start { pipeline } => {
                state.lock().await.start_pipeline(pipeline);
            }
            common::runner::PipelineMessage::Finish { pipeline, error } => {
                state.lock().await.finish_pipeline(pipeline, error);
            }
            common::runner::PipelineMessage::JobPending { pipeline, job_id } => {
                state
                    .lock()
                    .await
                    .set_job_status(pipeline, job_id, JobStatus::Pending);
            }
            common::runner::PipelineMessage::JobProgress {
                pipeline,
                job_id,
                step,
            } => {
                state
                    .lock()
                    .await
                    .set_job_status(pipeline, job_id, JobStatus::Running { step });
            }
            common::runner::PipelineMessage::JobFinished { pipeline, job_id } => {
                state
                    .lock()
                    .await
                    .set_job_status(pipeline, job_id, JobStatus::Finished);
            }
            common::runner::PipelineMessage::Log {
                pipeline,
                job_id,
                t,
                text,
                timestamp,
            } => {
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
                    common::runner::LogType::Regular => println!("{}", text.trim_end()),
                    common::runner::LogType::Error => {
                        println!(
                            "{}{}{}",
                            color::Fg(color::Red),
                            text.trim_end(),
                            style::Reset
                        )
                    }
                    common::runner::LogType::Warning => {
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

    Ok(())
}
