use std::{path::PathBuf, pin::Pin};

use thiserror::Error;
use warp::{Filter, Future};

use log::*;

use crate::lib::{git, utils::expand_home};

use super::{
    config::{Config, ConfigError},
    filters,
    git::GitError,
};

pub struct App {
    config: Config,
}

#[derive(Error, Debug)]
pub enum RunnerError {
    #[error("Failed to load config: {0}")]
    ConfigLoadError(#[from] ConfigError),
}

impl App {
    pub async fn init() -> Result<App, RunnerError> {
        pretty_env_logger::init();

        let app = App {
            config: Config::load("../tests/static/config".into()).await?,
        };

        info!("Loaded config: {:?}", app.config);

        Ok(app)
    }

    pub async fn run(self) {
        let api = filters::runner();
        let routes = api.with(warp::log("runner"));

        let mut git_tasks = Vec::new();

        for (id, repo) in self.config.repos.iter() {
            // info!("Cloning repo {}", id);
            // let task = git::clone_ssh(
            //     // TODO: Support http
            //     repo.source.strip_prefix("ssh://").unwrap().to_string(),
            //     expand_home("~/.microci/repos").join(id),
            // );

            // info!("Pulling repo {}", id);
            // let task = git::pull_ssh(expand_home("~/.microci/repos").join(id), "main".to_string());

            // info!("Commiting in repo {}", id);
            // let task = git::commit_all(
            //     expand_home("~/.microci/repos").join(id),
            //     String::from("Msg from microci"),
            // );

            info!("Push in repo {}", id);
            let task = git::push_ssh(
                expand_home("~/.microci/repos").join(id),
                String::from("main"),
            );

            git_tasks.push(task);
        }

        for task in git_tasks.into_iter() {
            task.await.unwrap();
        }

        // warp::serve(routes).run(([127, 0, 0, 1], 3002)).await;
    }
}
