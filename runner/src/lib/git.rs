use std::{
    fmt::Display,
    path::{Path, PathBuf},
};

use thiserror::Error;

use git2::{self, Cred, Direction, PushOptions, RemoteCallbacks, Repository};
use std::env;

use super::utils::expand_home;

use log::*;


// TODO: All of these quite hell. Use `gix` crate instead?

#[derive(Debug, Error)]
pub struct GitError(#[from] git2::Error);

type ChangedFiles = Vec<String>;

impl Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub async fn clone_ssh(repo: String, path: PathBuf) -> Result<(), GitError> {
    tokio::spawn(async move { clone_ssh_impl(repo, path) })
        .await
        .unwrap()
        .map_err(|e| e.into())
}

pub async fn check_exists(path: PathBuf) -> Result<bool, GitError> {
    Ok(
        tokio::spawn(async move { git2::Repository::open(path).is_ok() })
            .await
            .unwrap(),
    )
}

pub async fn pull_ssh(path: PathBuf, branch: String) -> Result<ChangedFiles, GitError> {
    tokio::spawn(async move { pull_ssh_impl(path, branch) })
        .await
        .unwrap()
        .map_err(|e| e.into())
}

pub async fn commit_all(path: PathBuf, msg: String) -> Result<(), GitError> {
    tokio::spawn(async move { commit_all_impl(path, msg) })
        .await
        .unwrap()
        .map_err(|e| e.into())
}

pub async fn push_ssh(path: PathBuf, branch: String) -> Result<(), GitError> {
    tokio::spawn(async move { push_ssh_impl(path, branch) })
        .await
        .unwrap()
        .map_err(|e| e.into())
}

pub fn clone_ssh_impl(repo: String, path: PathBuf) -> Result<(), git2::Error> {
    let mut callbacks = RemoteCallbacks::new();
    utils::with_ssh(&mut callbacks);

    let mut fo = git2::FetchOptions::new();
    fo.remote_callbacks(callbacks);

    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fo);

    builder.clone(&repo, path.as_path())?;

    Ok(())
}

fn pull_ssh_impl(path: PathBuf, branch: String) -> Result<ChangedFiles, git2::Error> {
    let changes = pull::run(path.clone(), branch.clone())?;
    move_branch(&Repository::open(path)?, &branch)?;

    Ok(changes)
}

fn push_ssh_impl(path: PathBuf, branch: String) -> Result<(), GitError> {
    let repo = Repository::open(path)?;
    move_branch(&repo, &branch)?;

    // FIXME: Optional remote?
    let mut remote = repo.find_remote("origin")?;

    let mut callbacks = RemoteCallbacks::new();
    utils::with_ssh(&mut callbacks);

    let mut po = PushOptions::new();
    po.remote_callbacks(callbacks);

    // NOTE: To force push prefix ref with "+"
    // Now assuming I will not have to force push from CI
    let refs = format!("refs/heads/{0}:refs/heads/{0}", branch);
    remote.push(&[refs], Some(&mut po))?;

    Ok(())
}

fn commit_all_impl(path: PathBuf, msg: String) -> Result<(), git2::Error> {
    // FIXME: Seems working, but the hell this is ugly
    let repo = Repository::open(path)?;
    let mut index = repo.index()?;

    index.add_all(["."], git2::IndexAddOption::DEFAULT, None)?;
    index.write()?;

    let head_commit = repo.reference_to_annotated_commit(&repo.head()?)?;
    let local_commit = repo.find_commit(head_commit.id())?;
    let result_tree = repo.find_tree(index.write_tree()?)?;
    let sig = repo.signature()?;

    let count_changes = repo
        .diff_tree_to_tree(Some(&local_commit.tree()?), Some(&result_tree), None)?
        .deltas()
        .len();
    if count_changes == 0 {
        info!("Nothing to commit");
        return Ok(());
    }

    let _commit = repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        &msg,
        &result_tree,
        &[&local_commit],
    )?;

    Ok(())
}

fn move_branch(repo: &Repository, branch: &str) -> Result<(), git2::Error> {
    let commit = repo.reference_to_annotated_commit(&repo.head()?)?;
    let mut b = repo.find_branch(branch, git2::BranchType::Local)?;

    b.get_mut()
        .set_target(commit.id(), &format!("Updating {} branch", branch))?;

    Ok(())
}

// https://github.com/rust-lang/git2-rs/blob/master/examples/pull.rs
mod pull {
    use std::path::PathBuf;

    use git2::{AnnotatedCommit, Diff, DiffFile, Repository};

    use log::*;

    pub fn run(path: PathBuf, branch: String) -> Result<super::ChangedFiles, git2::Error> {
        let remote_name = "origin";
        let repo = Repository::open(path)?;

        let old_id = repo.reference_to_annotated_commit(&repo.head()?)?.id();

        let mut remote = repo.find_remote(remote_name)?;
        let fetch_commit = do_fetch(&repo, &[&branch], &mut remote)?;
        do_merge(&repo, &branch, fetch_commit)?;

        let new_id = repo.reference_to_annotated_commit(&repo.head()?)?.id();

        let old_tree = repo.find_commit(old_id)?.tree()?;
        let new_tree = repo.find_commit(new_id)?.tree()?;

        let diff = repo.diff_tree_to_tree(Some(&old_tree), Some(&new_tree), None)?;
        info!("Diff stats: {:?}", diff.stats()?);

        let mut changed_files = Vec::new();

        // FIXME: Does not actually all changed files
        // For example after force push
        for delta in diff.deltas() {
            match delta.new_file().path() {
                None => warn!("Cannot get path file for one of the diff files"),
                Some(path) => changed_files.push(path.to_string_lossy().to_string()),
            }
        }

        Ok(changed_files)
    }

    fn do_merge(
        repo: &Repository,
        branch: &str,
        fetch_commit: AnnotatedCommit,
    ) -> Result<(), git2::Error> {
        let analysis = repo.merge_analysis(&[&fetch_commit])?;

        if analysis.0.is_fast_forward() {
            info!("Doing a fast forward");

            let refname = format!("refs/head/{}", branch);
            match repo.find_reference(&refname) {
                Ok(mut r) => {
                    fast_forward(repo, &mut r, &fetch_commit)?;
                }
                Err(_) => {
                    repo.reference(
                        &refname,
                        fetch_commit.id(),
                        true,
                        &format!("Setting {} to {}", branch, fetch_commit.id()),
                    );
                    repo.set_head(&refname)?;
                    repo.checkout_head(Some(
                        git2::build::CheckoutBuilder::default()
                            .allow_conflicts(true)
                            .conflict_style_merge(true)
                            .force(),
                    ));
                }
            }
        } else if analysis.0.is_normal() {
            let head_commit = repo.reference_to_annotated_commit(&repo.head()?)?;
            normal_merge(&repo, head_commit, &fetch_commit)?;
        } else {
            info!("Nothing to do...");
        }

        Ok(())
    }

    fn normal_merge(
        repo: &&Repository,
        head_commit: AnnotatedCommit,
        fetch_commit: &AnnotatedCommit,
    ) -> Result<(), git2::Error> {
        let local_tree = repo.find_commit(head_commit.id())?.tree()?;
        let remote_tree = repo.find_commit(fetch_commit.id())?.tree()?;
        let ancestor = repo
            .find_commit(repo.merge_base(head_commit.id(), fetch_commit.id())?)?
            .tree()?;
        let mut idx = repo.merge_trees(&ancestor, &local_tree, &remote_tree, None)?;

        if idx.has_conflicts() {
            error!("Merge confilts detected...");
            repo.checkout_index(Some(&mut idx), None)?;
            return Err(git2::Error::new(
                git2::ErrorCode::Conflict,
                git2::ErrorClass::Merge,
                "Merge conflict",
            ));
        }

        let result_tree = repo.find_tree(idx.write_tree_to(repo)?)?;

        let msg = format!("Merge: {} into {}", fetch_commit.id(), head_commit.id());
        let sig = repo.signature()?;
        let local_commit = repo.find_commit(head_commit.id())?;
        let remote_commit = repo.find_commit(fetch_commit.id())?;

        let _merge_commit = repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &msg,
            &result_tree,
            &[&local_commit, &remote_commit],
        )?;

        repo.checkout_head(None)?;
        Ok(())
    }

    fn fast_forward(
        repo: &Repository,
        r: &mut git2::Reference,
        fetch_commit: &AnnotatedCommit,
    ) -> Result<(), git2::Error> {
        let name = match r.name() {
            Some(s) => s.to_string(),
            None => String::from_utf8_lossy(r.name_bytes()).to_string(),
        };

        let msg = format!(
            "Fast-Forward: Setting {} to id: {}",
            name,
            fetch_commit.id()
        );
        info!("{}", msg);

        r.set_target(fetch_commit.id(), &msg)?;
        repo.set_head(&name)?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;

        Ok(())
    }

    fn do_fetch<'a>(
        repo: &'a Repository,
        refs: &[&str],
        remote: &mut git2::Remote,
    ) -> Result<AnnotatedCommit<'a>, git2::Error> {
        let mut callbacks = git2::RemoteCallbacks::new();
        super::utils::with_ssh(&mut callbacks);

        // TODO: Print progress?

        let mut fo = git2::FetchOptions::new();
        fo.remote_callbacks(callbacks);
        fo.download_tags(git2::AutotagOption::All);

        info!("Fetching {} for repo", remote.name().unwrap());
        remote.fetch(refs, Some(&mut fo), None)?;

        // TODO: Print stats?

        let fetch_head = repo.find_reference("FETCH_HEAD")?;
        Ok(repo.reference_to_annotated_commit(&fetch_head)?)
    }
}

mod utils {
    use git2::{Cred, RemoteCallbacks};

    use crate::lib::utils::expand_home;

    pub fn with_ssh(callbacks: &mut RemoteCallbacks) {
        callbacks.credentials(
            |_url, username_from_url, _allowed_types| -> Result<Cred, git2::Error> {
                Cred::ssh_key(
                    username_from_url.unwrap(),
                    None,
                    expand_home("~/.ssh/id_rsa").as_path(),
                    None,
                )
            },
        );
    }
}
