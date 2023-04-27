use std::{path::PathBuf, sync::Arc};

use common::state::State;
use runner_lib::context::ConfigsSource;
use tokio::sync::Mutex;
use warp::Filter;

use log::*;

use clap::{Args, Parser};

use super::filters;
use runner_lib::{config, context};

use runner_lib::call_context::Deps;

#[derive(Parser, Debug)]
#[command(about)]
struct RunnerArgs {
    /// TCP port to run on
    #[arg(short, long, default_value_t = 3002)]
    port: u16,

    /// Environment identifier to use for parameters
    #[arg(long, default_value_t = String::from("default"))]
    env: String,

    /// Use syslog for logs
    #[arg(long, default_value_t = false)]
    syslog: bool,

    /// Configs source
    #[command(flatten)]
    config: Config,
}

#[derive(Args, Debug)]
struct Config {
    /// Type of source with config
    #[command(flatten)]
    source: ConfigSource,

    /// Path to projects file
    #[arg(long, requires = "config")]
    projects: Option<PathBuf>,

    /// Url to clone repo from
    #[arg(short, long, requires = "config_repo")]
    url: Option<String>,

    /// Path inside repo with configs. Must contains uci.yaml
    #[arg(long, requires = "config_repo", default_value_t = String::from(".uci/"))]
    prefix: String,
}

#[derive(Args, Debug)]
#[group(required = true, multiple = false)]
struct ConfigSource {
    /// Path to config file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Path to repo with configs
    #[arg(long)]
    config_repo: Option<PathBuf>,
}

pub struct App {
    port: u16,
    configs_source: ConfigsSource,
    projects: Option<PathBuf>,
    env: String,
}

impl App {
    pub async fn init() -> Result<App, anyhow::Error> {
        let args: RunnerArgs = RunnerArgs::parse();

        if args.syslog {
            syslog::init_unix(syslog::Facility::LOG_USER, log::LevelFilter::Debug)
                .expect("Failed to initialize syslog");
        } else {
            pretty_env_logger::init_timed();
        }

        let (configs_source, projects): (ConfigsSource, Option<PathBuf>) =
            if let Some(config) = args.config.source.config {
                (ConfigsSource::Explicit { config }, args.config.projects)
            } else if let Some(path) = args.config.source.config_repo {
                let projects_path = path.join(&args.config.prefix).join("projects.yaml");
                let projects = if projects_path.exists() {
                    Some(projects_path)
                } else {
                    None
                };

                (
                    ConfigsSource::Repo {
                        url: args.config.url,
                        prefix: args.config.prefix,
                        path,
                    },
                    projects,
                )
            } else {
                unreachable!()
            };

        Ok(App {
            port: args.port,
            env: args.env,
            projects,
            configs_source,
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

        let docker = worker_lib::docker::Docker::init()?;
        let maybe_executor = worker_lib::executor::Executor::new().await?;

        state.set_named("worker", &());
        state.set_owned(docker);
        state.set_owned(maybe_executor);

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
            context::Context::new(projects_store, self.configs_source).await?
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
