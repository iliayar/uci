use bytes::Bytes;
use runner_lib::{
    call_context::{self, CallContext},
    config,
};

use serde::{Deserialize, Serialize};

use crate::filters::{validate_hmac_sha256, with_call_context, AuthRejection};

use reqwest::StatusCode;
use warp::Filter;

use log::*;

pub fn filter(
    deps: call_context::Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    api_call(deps.clone())
        .or(gitlab_webhook(deps.clone()))
        .or(github_webhook(deps))
}

pub fn api_call(
    deps: call_context::Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(warp::path!("update"))
        .and(with_call_context(deps))
        .and(warp::body::json::<models::UpdateRepoBody>())
        .and(warp::post())
        .and_then(update_repo)
}

#[derive(Serialize, Deserialize)]
struct Query {
    project_id: String,
    repo_id: String,
    dry_run: Option<bool>,
    update_only: Option<bool>,
}

pub fn gitlab_webhook(
    deps: call_context::Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(warp::path!("gitlab" / "update"))
        .and(warp::header("X-Gitlab-Token"))
        .map(move |token: String| CallContext::for_handler(Some(token), deps.clone()))
        .and(
            warp::query::<Query>().map(|query: Query| models::UpdateRepoBody {
                project_id: query.project_id,
                repo_id: query.repo_id,
                artifact_id: None,
                dry_run: query.dry_run,
                update_only: query.update_only,
            }),
        )
        .and(warp::post())
        .and_then(update_repo)
}

pub fn github_webhook(
    deps: call_context::Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(warp::path!("github" / "update"))
        .and(with_validate_github(deps.clone()))
        .map(move |token: Option<String>| CallContext::for_handler(token, deps.clone()))
        .and(
            warp::query::<Query>().map(|query: Query| models::UpdateRepoBody {
                project_id: query.project_id,
                repo_id: query.repo_id,
                artifact_id: None,
                dry_run: query.dry_run,
                update_only: query.update_only,
            }),
        )
        .and(warp::post())
        .and_then(update_repo)
}

fn with_validate_github(
    deps: call_context::Deps,
) -> impl Filter<Extract = (Option<String>,), Error = warp::Rejection> + Clone {
    warp::body::bytes()
        .and(warp::header("x-hub-signature-256"))
        .and_then(move |body: Bytes, header: String| {
            let deps = deps.clone();
            async move {
                let config = deps.context.config().await;
                // FIXME: From where to get secret better?
                let secret = if let Some(secret) = config.secrets.get("webhook-secret") {
                    Some(secret.clone())
                } else {
                    warn!("No secret webhook-secret to check github webhook");
                    None
                };

                validate_hmac_sha256(header, secret, body).await
            }
        })
}

async fn update_repo(
    mut call_context: call_context::CallContext,
    models::UpdateRepoBody {
        project_id,
        repo_id,
        artifact_id,
        dry_run,
        update_only,
    }: models::UpdateRepoBody,
) -> Result<impl warp::Reply, warp::Rejection> {
    if !call_context
        .check_permissions(Some(&project_id), config::permissions::ActionType::Write)
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
            .update_repo(
                &project_id,
                &repo_id,
                artifact,
                dry_run.unwrap_or(false),
                update_only.unwrap_or(false),
            )
            .await
        {
            error!("Updating repo failed: {}", err)
        }
        call_context.finish_run().await;
    });

    Ok(warp::reply::with_status(
        warp::reply::json(&models::ContinueReponse { run_id }),
        StatusCode::ACCEPTED,
    ))
}
