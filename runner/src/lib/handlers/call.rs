use crate::lib::{
    config::{self, ActionEvent, ActionType},
    filters::{with_call_context, CallContext, ContextPtr, InternalServerError, Unauthorized},
};

use reqwest::StatusCode;
use warp::Filter;

use log::*;

pub fn filter<PM: config::ProjectsManager>(
    context: ContextPtr<PM>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_call_context(context))
        .and(warp::path!("call" / String / String))
        .and(warp::post())
        .and_then(call)
}

async fn call<PM: config::ProjectsManager>(
    call_context: CallContext<PM>,
    project_id: String,
    trigger_id: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    info!("Running trigger {} for project {}", trigger_id, project_id);

    match call_context.context.get_project_info(&project_id).await {
        Ok(project) => {
            if !project.check_allowed_token(call_context.token, ActionType::Execute) {
                return Err(warp::reject::custom(Unauthorized::TokenIsUnauthorized));
            }
        }
        Err(err) => {
            return Err(warp::reject::custom(InternalServerError::Error(
                err.to_string(),
            )));
        }
    }

    // TODO: Trigger project
    // super::trigger_projects_impl(
    //     call_context,
    //     ActionEvent::DirectCall {
    //         project_id: project_id.clone(),
    //         trigger_id: trigger_id.clone(),
    //     },
    // )
    // .await;

    Ok(StatusCode::OK)
}
