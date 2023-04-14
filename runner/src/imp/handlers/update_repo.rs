use crate::imp::{
    config,
    filters::{with_call_context, AuthRejection, Deps},
};

use reqwest::StatusCode;
use warp::Filter;

use log::*;

pub fn filter<PM: config::ProjectsManager + 'static>(
    context: Deps<PM>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_call_context(context))
        .and(warp::path!("update" / String / String))
        .and(warp::post())
        .and_then(update_repo)
}

async fn update_repo<PM: config::ProjectsManager + 'static>(
    mut call_context: super::CallContext<PM>,
    project_id: String,
    repo_id: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    if !call_context
        .check_permissions(Some(&project_id), config::ActionType::Write)
        .await
    {
        return Err(warp::reject::custom(AuthRejection::TokenIsUnauthorized));
    }
    info!("Updating repo {}", repo_id);

    let client_id = call_context.init_ws().await;
    tokio::spawn(async move {
        if let Err(err) = call_context.update_repo(&project_id, &repo_id).await {
            error!("Updating repo failed: {}", err)
        }
	call_context.finish_ws().await;
    });

    Ok(warp::reply::with_status(
        warp::reply::json(&common::runner::ContinueReponse { client_id }),
        StatusCode::ACCEPTED,
    ))
}
