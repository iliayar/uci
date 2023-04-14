use std::{collections::HashMap, convert::Infallible, fmt::Debug, sync::Arc};
use tokio::sync::Mutex;
use warp::{Filter, Rejection};

use super::{
    config,
    context::Context,
    handlers::{self, WsClient},
};
use warp::hyper::StatusCode;

pub type ContextPtr<PM> = Arc<Context<PM>>;

pub struct Deps<PM: config::ProjectsManager> {
    pub context: Arc<Context<PM>>,
    pub ws_clients: Arc<Mutex<HashMap<String, WsClient>>>,
}

impl<PM: config::ProjectsManager> Clone for Deps<PM> {
    fn clone(&self) -> Self {
        Self {
            context: self.context.clone(),
            ws_clients: self.ws_clients.clone(),
        }
    }
}

pub fn runner<PM: config::ProjectsManager + 'static>(
    context: Context<PM>,
) -> impl Filter<Extract = impl warp::Reply, Error = Rejection> + Clone {
    let deps = Deps {
        context: Arc::new(context),
        ws_clients: Arc::new(Mutex::new(HashMap::new())),
    };

    ping()
        .or(handlers::call::filter(deps.clone()))
        .or(handlers::reload_config::filter(deps.clone()))
        .or(handlers::update_repo::filter(deps.clone()))
        .or(handlers::list_projects::filter(deps.clone()))
        .or(handlers::ws::filter(deps))
        .recover(report_rejection)
}

pub fn ping() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("ping").and(warp::get()).map(|| StatusCode::OK)
}

pub fn with_call_context<PM: config::ProjectsManager>(
    deps: Deps<PM>,
) -> impl Filter<Extract = (handlers::CallContext<PM>,), Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_validation())
        .and(with_deps(deps))
        .map(handlers::CallContext::for_handler)
}

pub fn with_deps<PM: config::ProjectsManager>(
    deps: Deps<PM>,
) -> impl Filter<Extract = (Deps<PM>,), Error = Infallible> + Clone {
    warp::any().map(move || deps.clone())
}

#[derive(Debug)]
pub enum AuthRejection {
    UnsupportedAuthorizationMethod(String),
    MethodIsNotSepcified,
    TokenIsNotSpecified,
    TokenIsUnauthorized,
}

impl warp::reject::Reject for AuthRejection {}

// FIXME: Make one error, meaningfull
#[derive(Debug)]
pub enum InternalServerError {
    Error(String),
}

impl warp::reject::Reject for InternalServerError {}

pub fn with_validation() -> impl Filter<Extract = (Option<String>,), Error = Rejection> + Clone {
    warp::header::optional("Authorization").and_then(|auth: Option<String>| async move {
        if let Some(auth) = auth {
            let mut split = auth.split_whitespace();
            if let Some(method) = split.next() {
                if method != "Api-Key" {
                    Err(warp::reject::custom(
                        AuthRejection::UnsupportedAuthorizationMethod(method.to_string()),
                    ))
                } else if let Some(token) = split.next() {
                    Ok(Some(token.to_string()))
                } else {
                    Err(warp::reject::custom(AuthRejection::TokenIsNotSpecified))
                }
            } else {
                Err(warp::reject::custom(AuthRejection::MethodIsNotSepcified))
            }
        } else {
            Ok(None)
        }
    })
}

pub async fn report_rejection(r: Rejection) -> Result<impl warp::Reply, Rejection> {
    if let Some(auth_error) = r.find::<AuthRejection>() {
        let message = match auth_error {
            AuthRejection::UnsupportedAuthorizationMethod(method) => {
                format!("Unsupported auth method {}", method)
            }
            AuthRejection::MethodIsNotSepcified => "Auth method is not specified".to_string(),
            AuthRejection::TokenIsNotSpecified => "Auth token is not specified".to_string(),
            AuthRejection::TokenIsUnauthorized => {
                "Specified token is unauthrized for this action".to_string()
            }
        };
        return Ok(warp::reply::with_status(
            warp::reply::json(&common::runner::ErrorResponse { message }),
            StatusCode::UNAUTHORIZED,
        ));
    } else if let Some(internal_server_error) = r.find::<InternalServerError>() {
        let message = match internal_server_error {
            InternalServerError::Error(err) => err.clone(),
        };
        return Ok(warp::reply::with_status(
            warp::reply::json(&common::runner::ErrorResponse { message }),
            StatusCode::INTERNAL_SERVER_ERROR,
        ));
    }

    Err(r)
}
