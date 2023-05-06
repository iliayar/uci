#![allow(unused_variables)]
#![allow(dead_code)]

mod cli;
mod config;
mod execute;
mod prompts;
mod runner;
mod select;
mod utils;

use clap::Parser;

use termion::{color, style};

use log::*;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = cli::Cli::parse();

    let config = simplelog::ConfigBuilder::new().build();
    simplelog::TermLogger::init(
        get_log_level(args.verbose),
        config,
        simplelog::TerminalMode::Stderr,
        simplelog::ColorChoice::Auto,
    )
    .expect("Failed to init logger");

    debug!("Arguments parsed");
    debug!("Loading config");
    let project = if let Some(project) = args.project {
        Some(Some(project))
    } else if args.select_project {
        Some(None)
    } else {
        None
    };
    let config = match config::Config::load(args.config_path.into(), args.env, project).await {
        Err(err) => {
            error!("Failed to load config, using default: {}", err);
            config::Config::default()
        }
        Ok(config) => config,
    };
    debug!("Loaded config {:?}", config);

    if let Err(err) = execute::execute(&config, args.command).await {
        match err {
            execute::ExecuteError::Fatal(err) => {
                eprintln!("{}{}{}", color::Fg(color::Red), err, style::Reset)
            }
            execute::ExecuteError::Warning(err) => {
                eprintln!("{}{}{}", color::Fg(color::Yellow), err, style::Reset)
            }
            execute::ExecuteError::Other(err) => {
                eprintln!("{}{}{}", color::Fg(color::LightRed), err, style::Reset)
            }
            execute::ExecuteError::Interrupted => {
                return;
            }
        }
    }
}

fn get_log_level(verbose: u8) -> log::LevelFilter {
    match verbose {
        0 => log::LevelFilter::Off,
        1 => log::LevelFilter::Warn,
        2 => log::LevelFilter::Info,
        3 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    }
}
