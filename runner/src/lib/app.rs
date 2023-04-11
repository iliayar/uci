use std::{path::PathBuf, pin::Pin, sync::Arc};

use thiserror::Error;
use warp::{Filter, Future};

use log::*;

use clap::Parser;

use super::{config, context, filters, git};

#[derive(Parser, Debug)]
#[command(about)]
struct Args {
    /// Path to config file
    #[arg(short, long)]
    config: PathBuf,

    /// Path to projects file
    #[arg(long)]
    projects: Option<PathBuf>,

    /// TCP port to run on
    #[arg(short, long, default_value_t = 3002)]
    port: u16,

    /// Do not use external worker, run pipelines in the same process
    #[arg(long, default_value_t = false)]
    worker: bool,

    /// Environment identifier to use for parameters
    #[arg(long, default_value_t = String::from("default"))]
    env: String,
}

pub struct App {
    port: u16,
    config: PathBuf,
    projects: Option<PathBuf>,
    worker: bool,
    env: String,
}

#[derive(Error, Debug)]
pub enum RunnerError {
    #[error("Failed to load config: {0}")]
    ConfigLoadError(#[from] config::LoadConfigError),

    #[error("Failed to create context: {0}")]
    ContextError(#[from] context::ContextError),

    #[error("Failed to init docker: {0}")]
    DockerError(#[from] worker_lib::docker::DockerError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl App {
    pub async fn init() -> Result<App, RunnerError> {
        pretty_env_logger::init_timed();

        let args = Args::parse();

        Ok(App {
            port: args.port,
            config: args.config,
            projects: args.projects,
            worker: args.worker,
            env: args.env,
        })
    }

    pub async fn run(self) {
        match self.run_impl().await {
            Ok(_) => {}
            Err(err) => {
                error!("Failed to run app, exiting: {}", err);
            }
        }
    }

    async fn run_impl(self) -> Result<(), RunnerError> {
        let worker_context = if self.worker {
            let docker = worker_lib::docker::Docker::init()?;
            Some(worker_lib::context::Context::new(docker))
        } else {
            None
        };

        let context = if let Some(projects) = self.projects {
            let mut context = config::LoadContext::default();
            context.set_named("projects_config", &projects);
            let manager = config::StaticProjects::default();
            let projects_store = config::ProjectsStore::with_manager(manager).await?;
            context::Context::new(projects_store, worker_context, self.config, self.env).await?
        } else {
            unimplemented!()
        };

        // let call_context = super::filters::CallContext {
        //     token: None,
        //     check_permisions: false,
        //     worker_context: worker_context.clone(),
        //     store: context_store.clone(),
        //     ws: None,
        // };
        // tokio::spawn(super::handlers::trigger_projects_impl(
        //     call_context,
        //     super::config::ActionEvent::ConfigReloaded,
        // ));

        let api = filters::runner(context);
        let routes = api.with(warp::log("runner"));
        warp::serve(routes).run(([127, 0, 0, 1], self.port)).await;
        Ok(())
    }
}
