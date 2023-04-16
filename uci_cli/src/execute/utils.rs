use std::collections::HashMap;
use std::io::Write;

use crate::{runner::WsClient, utils::Spinner};

use termion::{clear, color, scroll, style};

pub async fn print_clone_repos(ws_client: &mut WsClient) -> Result<(), super::ExecuteError> {
    match ws_client
        .receive::<common::runner::CloneMissingRepos>()
        .await
    {
        Some(common::runner::CloneMissingRepos::Begin) => {}
        _ => {
            return Err(super::ExecuteError::Warning(
                "Expect begin message for clone missing repos".to_string(),
            ));
        }
    }

    enum Status {
        InProgress,
        Done,
    }

    let mut repos_to_clone: HashMap<String, Status> = HashMap::new();
    let mut spinner = Spinner::new();

    loop {
        if let Some(message) = ws_client
            .try_receive::<common::runner::CloneMissingRepos>()
            .await
        {
            match message {
                common::runner::CloneMissingRepos::Begin => unreachable!(),
                common::runner::CloneMissingRepos::ClonningRepo { repo_id } => {
                    if repos_to_clone.is_empty() {
                        println!(
                            "{}Clonning missing repos:{}",
                            color::Fg(color::Blue),
                            style::Reset
                        );
                    }
                    repos_to_clone.insert(repo_id, Status::InProgress);
                }
                common::runner::CloneMissingRepos::RepoCloned { repo_id } => {
                    repos_to_clone.insert(repo_id, Status::Done);
                }
                common::runner::CloneMissingRepos::Finish => {
                    if !repos_to_clone.is_empty() {
                        print!("{}{}", scroll::Down(1), clear::CurrentLine);
                        std::io::stdout()
                            .flush()
                            .map_err(|err| super::ExecuteError::Fatal(err.to_string()))?;
                        println!(
                            "{}Missing repos cloned{}",
                            color::Fg(color::Green),
                            style::Reset
                        );
                    }
                    break;
                }
            }
        }

        let ch = spinner.next();
        for (repo, status) in repos_to_clone.iter() {
            match status {
                Status::InProgress => {
                    println!(
                        "  [{}{}{}] {}",
                        color::Fg(color::Blue),
                        ch,
                        style::Reset,
                        repo
                    );
                }
                Status::Done => {
                    println!(
                        "  [{}DONE{}] {}",
                        color::Fg(color::Green),
                        style::Reset,
                        repo
                    );
                }
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        if !repos_to_clone.is_empty() {
            print!("{}", scroll::Down(repos_to_clone.len() as u16));
            std::io::stdout()
                .flush()
                .map_err(|err| super::ExecuteError::Fatal(err.to_string()))?;
        }
    }

    Ok(())
}
