use std::{path::PathBuf, sync::Arc};

use common::state::State;
use tokio::sync::Mutex;
use warp::Filter;

use log::*;

use clap::Parser;

use super::filters;
use runner_lib::{config, context};

use runner_lib::call_context::Deps;

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

    /// Use syslog for logs
    #[arg(long, default_value_t = false)]
    syslog: bool,
}

pub struct App {
    port: u16,
    config: PathBuf,
    projects: Option<PathBuf>,
    worker: bool,
    env: String,
}

impl App {
    pub async fn init() -> Result<App, anyhow::Error> {
        let args = Args::parse();

        if args.syslog {
            syslog::init_unix(syslog::Facility::LOG_USER, log::LevelFilter::Debug)
                .expect("Failed to initialize syslog");
        } else {
            pretty_env_logger::init_timed();
        }

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

    async fn run_impl(self) -> Result<(), anyhow::Error> {
        let mut state = State::default();

        let mut maybe_docker = if self.worker {
            Some(worker_lib::docker::Docker::init()?)
        } else {
            None
        };

        let mut maybe_executor = if self.worker {
            Some(worker_lib::executor::Executor::new().await?)
        } else {
            None
        };

        if self.worker {
            state.set_named("worker", &());
            state.set_owned(maybe_docker.take().unwrap());
            state.set_owned(maybe_executor.take().unwrap());
        }
        state.set_named_owned("env", self.env);

        let context = if let Some(projects) = self.projects {
            let projects = match projects.canonicalize() {
                Ok(path) => path,
                Err(err) => {
                    return Err(anyhow::anyhow!("Bad projects config path: {}", err));
                }
            };
            let manager = config::StaticProjects::new(projects).await?;
            let projects_store = config::ProjectsStore::with_manager(manager).await?;
            context::Context::new(projects_store, self.config).await?
        } else {
            unimplemented!()
        };

        context.init(&state).await?;

        let deps = Deps {
            context: Arc::new(context),
            runs: Arc::new(Mutex::new(Default::default())),
            state: Arc::new(state),
        };
        let api = filters::runner(deps);
        let routes = api.with(warp::log("runner"));
        warp::serve(routes).run(([127, 0, 0, 1], self.port)).await;

        Ok(())
    }
}
