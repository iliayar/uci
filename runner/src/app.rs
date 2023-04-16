use std::path::PathBuf;

use warp::Filter;

use log::*;

use clap::Parser;

use runner_lib::{config, context};
use super::filters;

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

impl App {
    pub async fn init() -> Result<App, anyhow::Error> {
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

    async fn run_impl(self) -> Result<(), anyhow::Error> {
        let worker_context = if self.worker {
            let docker = worker_lib::docker::Docker::init()?;
            Some(worker_lib::context::Context::new(docker))
        } else {
            None
        };

        let context = if let Some(projects) = self.projects {
            let projects = match projects.canonicalize() {
                Ok(path) => path,
                Err(err) => {
                    return Err(anyhow::anyhow!("Bad projects config path: {}", err));
                }
            };
            let manager = config::StaticProjects::new(projects).await?;
            let projects_store = config::ProjectsStore::with_manager(manager).await?;
            context::Context::new(projects_store, worker_context, self.config, self.env).await?
        } else {
            unimplemented!()
        };
        context.init().await?;

        let api = filters::runner(context);
        let routes = api.with(warp::log("runner"));
        warp::serve(routes).run(([127, 0, 0, 1], self.port)).await;
        Ok(())
    }
}
