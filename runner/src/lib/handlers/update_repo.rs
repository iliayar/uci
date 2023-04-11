use crate::lib::{
    config::{self, ActionEvent, ActionType},
    filters::{with_call_context, CallContext, ContextPtr},
};

use reqwest::StatusCode;
use warp::Filter;

use log::*;

pub fn filter<PM: config::ProjectsManager>(
    context: ContextPtr<PM>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_call_context(context))
        .and(warp::path!("update" / String / String))
        .and(warp::post())
        .and_then(update_repo)
}

async fn update_repo<PM: config::ProjectsManager>(
    call_context: CallContext<PM>,
    project_id: String,
    repo: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    info!("Running repo {}", repo);
    super::trigger_projects_impl(
        call_context,
        ActionEvent::UpdateRepos {
            project_id,
            repos: vec![repo],
        },
    )
    .await;
    Ok(StatusCode::OK)
}
