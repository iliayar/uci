use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// uCI command line interface
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[clap(name = "uci")]
pub struct Cli {
    /// Environment to get parameters from config for
    #[arg(short, long, default_value_t = default_env())]
    pub env: String,

    /// Config path
    #[arg(short, long, default_value_t = default_config())]
    pub config_path: String,

    /// Verbose mode (-v, -vv, -vvv, etc.)
    #[clap(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Commands,
}

fn default_env() -> String {
    String::from("default")
}

fn default_config() -> String {
    let home = std::env::var("HOME").expect("HOME variable is not set. Specify config manually");
    PathBuf::from(home)
        .join(".microci/config.yaml")
        .to_string_lossy()
        .to_string()
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Perform actions on projects
    Projects {
        #[command(subcommand)]
        command: ProjectCommands,
    },

    /// Perform actions on global config
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum ProjectCommands {
    /// List projects
    List {},
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// List projects
    Reload {},
}
