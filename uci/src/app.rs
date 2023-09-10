use std::{path::PathBuf, sync::Arc};

use common::state::State;
use runner_lib::context::ConfigsSource;
use tokio::sync::Mutex;
use warp::Filter;

use log::*;

use clap::{Args, Parser};

use super::filters;
use runner_lib::{config, context};

use runner_lib::call_context::{ArtifactsStorage, Deps};

// FIXME: Move it to config maybe
const ARTIFACTS_PATH: &str = "/tmp/uci-artifacts";
const ARTIFACTS_LIMIT: usize = 5;

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

    /// Url to clone repo from
    #[arg(short, long, requires = "config_repo")]
    url: Option<String>,

    /// Path inside repo with configs. Must contains uci.yaml
    #[arg(long, requires = "config_repo", default_value_t = String::from(".uci/config"))]
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

        let configs_source: ConfigsSource = if let Some(config) = args.config.source.config {
            ConfigsSource::Explicit { config }
        } else if let Some(path) = args.config.source.config_repo {
            ConfigsSource::Repo {
                url: args.config.url,
                prefix: args.config.prefix,
                path,
            }
        } else {
            unreachable!()
        };

        Ok(App {
            port: args.port,
            env: args.env,
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

        state.set_owned(docker);
        state.set_owned(maybe_executor);

        let env = config::utils::Env(self.env);
        state.set_owned(env);

        let context = context::Context::new(&state, self.configs_source).await?;

        let deps = Deps {
            context: Arc::new(context),
            runs: Arc::new(Mutex::new(Default::default())),
            state: Arc::new(state),
            artifacts: ArtifactsStorage::new(PathBuf::from(ARTIFACTS_PATH), ARTIFACTS_LIMIT)
                .await?,
        };
        let api = filters::runner(deps);
        let routes = api.with(warp::log("runner"));
        warp::serve(routes).run(([0, 0, 0, 0], self.port)).await;

        Ok(())
    }
}
