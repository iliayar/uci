use std::convert::Infallible;

use log::*;
use warp::hyper::StatusCode;

use super::{context::Context, filters::ContextStore};

// fn check_permissions(
//     token: Option<String>,
//     actions: &super::config::MatchedActions,
//     config: &super::config::ServiceConfig,
// ) -> Result<(), warp::Rejection> {
//     if actions.check_allowed(token.as_ref(), config) {
//         Ok(())
//     } else {
//         Err(warp::reject::custom(
//             super::filters::Unauthorized::TokenIsUnauthorized,
//         ))
//     }
// }

async fn check_authorized<S: AsRef<str>, PS: AsRef<str>>(
    store: &ContextStore,
    token: Option<S>,
    project_id: Option<PS>,
    action: super::config::ActionType,
) -> Result<(), warp::Rejection> {
    if !store
        .context()
        .config()
        .await
        .service_config
        .check_allowed(token, project_id, action)
    {
        Err(warp::reject::custom(
            super::filters::Unauthorized::TokenIsUnauthorized,
        ))
    } else {
        Ok(())
    }
}

pub async fn call(
    token: Option<String>,
    project_id: String,
    trigger_id: String,
    store: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> Result<impl warp::Reply, warp::Rejection> {
    info!("Running trigger {} for project {}", trigger_id, project_id);
    check_authorized(
        &store,
        token.as_ref(),
        Some(&project_id),
        super::config::ActionType::Write,
    )
    .await?;

    let trigger = super::config::ActionEvent::DirectCall {
        project_id: project_id.clone(),
        trigger_id: trigger_id.clone(),
    };
    trigger_projects_impl(
        token,
        /* check_permissions */ true,
        trigger,
        store,
        worker_context,
    )
    .await;

    Ok(StatusCode::OK)
}

pub async fn update_repo(
    token: Option<String>,
    repo: String,
    store: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> Result<impl warp::Reply, warp::Rejection> {
    info!("Running repo {}", repo);
    let trigger = super::config::ActionEvent::UpdateRepos { repos: vec![repo] };
    trigger_projects_impl(
        token,
        /* check_permissions */ true,
        trigger,
        store,
        worker_context,
    )
    .await;

    Ok(StatusCode::OK)
}

pub async fn reload_config(
    token: Option<String>,
    store: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> Result<impl warp::Reply, warp::Rejection> {
    check_authorized::<_, &str>(
        &store,
        token.as_ref(),
        None,
        super::config::ActionType::Write,
    )
    .await?;

    match reload_config_impl(
        token,
        /* check_permissions */ true,
        store,
        worker_context,
    )
    .await
    {
        Ok(_) => Ok(warp::reply::with_status(
            warp::reply::json(&common::runner::EmptyResponse {}),
            StatusCode::OK,
        )),
        Err(err) => Ok(warp::reply::with_status(
            warp::reply::json(&common::runner::ErrorResponse {
                message: err.to_string(),
            }),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

pub async fn reload_project(
    token: Option<String>,
    project_id: String,
    store: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> Result<impl warp::Reply, warp::Rejection> {
    check_authorized::<_, &str>(
        &store,
        token.as_ref(),
        Some(&project_id),
        super::config::ActionType::Write,
    )
    .await?;

    match reload_project_impl(
        token,
        /* check_permissions */ true,
        project_id,
        store,
        worker_context,
    )
    .await
    {
        Ok(_) => Ok(warp::reply::with_status(
            warp::reply::json(&common::runner::EmptyResponse {}),
            StatusCode::OK,
        )),
        Err(err) => Ok(warp::reply::with_status(
            warp::reply::json(&common::runner::ErrorResponse {
                message: err.to_string(),
            }),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

async fn reload_config_impl(
    token: Option<String>,
    check_permissions: bool,
    store: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> Result<(), anyhow::Error> {
    store.context().reload_config().await?;

    let trigger = super::config::ActionEvent::ConfigReloaded;
    trigger_projects_impl(token, check_permissions, trigger, store, worker_context).await;

    Ok(())
}

async fn reload_project_impl(
    token: Option<String>,
    check_permissions: bool,
    project_id: String,
    store: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> Result<(), anyhow::Error> {
    store.context().reload_config().await?;

    let trigger = super::config::ActionEvent::ProjectReloaded { project_id };
    trigger_projects_impl(token, check_permissions, trigger, store, worker_context).await;

    Ok(())
}

pub async fn trigger_projects_impl(
    token: Option<String>,
    check_permissions: bool,
    trigger: super::config::ActionEvent,
    store: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) {
    tokio::spawn(async move {
        match trigger_projects_impl_result(token, check_permissions, trigger, store, worker_context)
            .await
        {
            Result::Err(err) => {
                error!("Failed to match actions: {}", err);
            }
            Result::Ok(_) => {}
        }
    });
}

pub async fn trigger_projects_impl_result(
    token: Option<String>,
    check_permissions: bool,
    trigger: super::config::ActionEvent,
    store: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> Result<(), anyhow::Error> {
    let mut matched = store
        .context()
        .config()
        .await
        .get_projects_actions(trigger)
        .await?;

    if matched.reload_config {
        if check_permissions
            && !store
                .context()
                .config()
                .await
                .service_config
                .check_allowed::<_, &str>(token.as_ref(), None, super::config::ActionType::Write)
        {
            return Err(anyhow::anyhow!(
                "Reloading config is not allowed, do nothing"
            ));
        }
    }

    // FIXME: Do it separately?
    if matched.reload_config || !matched.reload_projects.is_empty() {
        store.context().reload_config().await?;
        let new_matched = store
            .context()
            .config()
            .await
            .get_projects_actions(super::config::ActionEvent::ConfigReloaded)
            .await?;
        matched.merge(new_matched);
    }

    let mut new_matcheds = Vec::new();
    for project_id in matched.reload_projects.iter() {
        if check_permissions
            && !store.context().config().await.service_config.check_allowed(
                token.as_ref(),
                Some(&project_id),
                super::config::ActionType::Write,
            )
        {
            warn!("Not allowed to reload project {}, do nothing", project_id);
            continue;
        }

        let new_matched = store
            .context()
            .config()
            .await
            .get_projects_actions(super::config::ActionEvent::ProjectReloaded {
                project_id: project_id.clone(),
            })
            .await?;
        new_matcheds.push(new_matched);
    }
    for new_matched in new_matcheds.into_iter() {
        matched.merge(new_matched);
    }

    store
        .context()
        .config()
        .await
        .run_project_actions(token, check_permissions, worker_context, matched)
        .await?;

    Ok(())
}
