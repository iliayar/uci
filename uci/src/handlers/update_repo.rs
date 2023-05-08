use runner_lib::{
    call_context::{self, CallContext},
    config,
};

use serde::{Deserialize, Serialize};

use crate::filters::{with_call_context, AuthRejection};

use reqwest::StatusCode;
use warp::Filter;

use log::*;

pub fn filter(
    deps: call_context::Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    api_call(deps.clone()).or(gitlab_webhook(deps))
}

pub fn api_call(
    deps: call_context::Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_call_context(deps))
        .and(warp::path!("update"))
        .and(warp::body::json::<common::runner::UpdateRepoBody>())
        .and(warp::post())
        .and_then(update_repo)
}

#[derive(Serialize, Deserialize)]
struct Query {
    project_id: String,
    repo_id: String,
}

pub fn gitlab_webhook(
    deps: call_context::Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(warp::header("X-Gitlab-Token"))
        .map(move |token: String| CallContext::for_handler(Some(token), deps.clone()))
        .and(warp::path!("gitlab" / "update"))
        .and(
            warp::query::<Query>().map(|query: Query| common::runner::UpdateRepoBody {
                project_id: query.project_id,
                repo_id: query.repo_id,
                artifact_id: None,
            }),
        )
        .and(warp::post())
        .and_then(update_repo)
}

async fn update_repo(
    mut call_context: call_context::CallContext,
    common::runner::UpdateRepoBody {
        project_id,
        repo_id,
        artifact_id,
    }: common::runner::UpdateRepoBody,
) -> Result<impl warp::Reply, warp::Rejection> {
    if !call_context
        .check_permissions(Some(&project_id), config::ActionType::Write)
        .await
    {
        return Err(warp::reject::custom(AuthRejection::TokenIsUnauthorized));
    }
    info!("Updating repo {}", repo_id);

    let run_id = call_context.init_run_buffered().await;
    tokio::spawn(async move {
        call_context
            .wait_for_clients(std::time::Duration::from_secs(2))
            .await;
        let artifact = artifact_id.map(|id| call_context.artifacts.get_path(id));
        if let Err(err) = call_context
            .update_repo(&project_id, &repo_id, artifact)
            .await
        {
            error!("Updating repo failed: {}", err)
        }
        call_context.finish_run().await;
    });

    Ok(warp::reply::with_status(
        warp::reply::json(&common::runner::ContinueReponse { run_id }),
        StatusCode::ACCEPTED,
    ))
}
