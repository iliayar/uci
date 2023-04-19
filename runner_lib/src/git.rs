use std::{path::PathBuf, process::ExitStatus};

use tokio::process::Command;

use common::utils::{run_command_with_output, tempfile::TempFile};

use anyhow::anyhow;
use log::*;

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("{0}")]
    IOError(#[from] tokio::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type ChangedFiles = Vec<String>;

pub async fn clone(repo: String, path: PathBuf) -> Result<(), GitError> {
    git(
        PathBuf::from("."),
        &[
            String::from("clone"),
            repo,
            path.to_string_lossy().to_string(),
        ],
    )
    .await
    .map(|_| ())
}

pub async fn archive(path: PathBuf) -> Result<TempFile, GitError> {
    let tempfile = TempFile::dir().await?;
    let archive_path = tempfile.path.join("repo.tar.xz");
    git(
        path.clone(),
        &[
            String::from("archive"),
            String::from("--format"),
            String::from("tar.gz"),
            String::from("HEAD"),
            String::from("-o"),
            archive_path.to_string_lossy().to_string(),
        ],
    )
	.await?;
    Ok(tempfile)
}

pub async fn check_exists(path: PathBuf) -> Result<bool, GitError> {
    if !path.exists() {
        return Ok(false);
    }

    git_status(path.clone(), &[String::from("status")])
        .await
        .map(|s| s.success())
}

pub async fn pull(path: PathBuf, branch: String) -> Result<ChangedFiles, GitError> {
    let old_commit = current_commit(path.clone()).await?;

    git(path.clone(), &[String::from("fetch")]).await?;
    git(path.clone(), &[String::from("checkout"), branch.clone()]).await?;
    git(
        path.clone(),
        &[
            String::from("reset"),
            String::from("--hard"),
            format!("origin/{}", branch.clone()),
        ],
    )
    .await?;

    let new_commit = current_commit(path.clone()).await?;

    git_out(
        path.clone(),
        &[
            String::from("diff"),
            String::from("--name-only"),
            old_commit,
            new_commit,
        ],
    )
    .await
}

async fn current_commit(path: PathBuf) -> Result<String, GitError> {
    git_out(
        path,
        &[
            String::from("show-ref"),
            String::from("--hash"),
            String::from("HEAD"),
        ],
    )
    .await
    .map(|lines| lines[0].clone())
}

async fn git_out(path: PathBuf, args: &[String]) -> Result<Vec<String>, GitError> {
    let mut command = git_base(path)?;
    command.args(args);
    debug!("Executing git command: git {}", args.join(" "));

    let out = command.output().await?;
    Ok(String::from_utf8_lossy(&out.stdout)
        .to_string()
        .lines()
        .into_iter()
        .map(String::from)
        .collect())
}

async fn git(path: PathBuf, args: &[String]) -> Result<(), GitError> {
    let status = git_status(path, args).await?;

    if !status.success() {
        Err(anyhow!("Git finished with code {}", status).into())
    } else {
        Ok(())
    }
}

async fn git_status(path: PathBuf, args: &[String]) -> Result<ExitStatus, GitError> {
    let mut command = git_base(path)?;
    command.args(args);
    debug!("Executing git command: git {}", args.join(" "));

    let status = run_command_with_output(command).await?;

    Ok(status)
}

fn git_base(path: PathBuf) -> Result<Command, GitError> {
    // FIXME: Specify git binary in somewhere outside
    let mut command = Command::new("git");
    command.current_dir(path);

    Ok(command)
}
