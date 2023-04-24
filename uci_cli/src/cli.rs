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

    /// Perform actions on pipeline runs
    Runs {
        #[command(subcommand)]
        command: RunCommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum RunCommands {
    /// List runs
    List {
        #[clap(long)]
        project: Option<String>,

        #[clap(long)]
        pipeline: Option<String>,
    },

    /// List runs
    Logs {
        #[clap(short, long)]
        project: String,

        #[clap(long)]
        pipeline: String,

	#[clap(short, long)]
	run_id: String,

	/// Keep watching logs
	#[clap(short, long)]
	follow: bool,

	/// Print overall runs status bottom
	#[clap(short, long)]
	status: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ProjectCommands {
    /// List projects
    List {},

    /// Manage actions
    Actions {
        #[command(subcommand)]
        command: ActionCommand,
    },

    /// Manage repos
    Repos {
        #[command(subcommand)]
        command: RepoCommand,
    },

    /// Manage pipelines
    Pipelines {
        #[command(subcommand)]
        command: PipelineCommand,
    },

    /// Manage services
    Services {
        #[command(subcommand)]
        command: ServiceCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Reload config
    Reload {},
}

#[derive(Subcommand, Debug)]
pub enum ActionCommand {
    /// Call action directly
    Call {
        #[clap(short, long)]
        project: String,

        #[clap(short, long)]
        action: String,
    },

    /// List actions
    List {
        #[clap(short, long)]
        project: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum RepoCommand {
    /// Pull repo from remote
    Update {
        #[clap(short, long)]
        project: String,

        #[clap(short, long)]
        repo: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum PipelineCommand {
    /// List pipeliens
    List {
        #[clap(short, long)]
        project: String,
    },
}

fn default_logs_follow() -> bool {
    false
}

fn default_start_no_build() -> bool {
    false
}

#[derive(Subcommand, Debug)]
pub enum ServiceCommand {
    /// List services
    List {
        #[clap(short, long)]
        project: String,
    },

    /// List services
    Logs {
        #[clap(short, long)]
        project: String,

        #[clap(short, long)]
        service: Option<Vec<String>>,

        /// Keep watching logs
        #[clap(short, long, default_value_t = default_logs_follow())]
        follow: bool,

        /// Watch last TAIL logs if specified, otherwise from container start
        #[clap(short, long)]
        tail: Option<usize>,
    },

    /// Start services
    Start {
        #[clap(short, long)]
        project: String,

        #[clap(short, long)]
        service: Option<Vec<String>>,

        /// Do not build image before starting
        #[clap(long, default_value_t = default_start_no_build())]
        no_build: bool,
    },

    /// Stop services
    Stop {
        #[clap(short, long)]
        project: String,

        #[clap(short, long)]
        service: Option<Vec<String>>,
    },

    /// Stop services
    Restart {
        #[clap(short, long)]
        project: String,

        #[clap(short, long)]
        service: Option<Vec<String>>,

        /// Do not build image before starting
        #[clap(long, default_value_t = default_start_no_build())]
        no_build: bool,
    },
}
