use std::{convert::Infallible, sync::Arc};
use warp::{Filter, Rejection};

use super::{
    context::{Context, ContextError},
    handlers,
};
use warp::hyper::StatusCode;

#[derive(Clone)]
pub struct ContextStore {
    context: Arc<Context>,
}

impl ContextStore {
    pub fn new(context: Context) -> ContextStore {
        ContextStore {
            context: Arc::new(context),
        }
    }

    pub fn context(&self) -> &Context {
        self.context.as_ref()
    }
}

pub fn runner(
    context_store: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> impl Filter<Extract = impl warp::Reply, Error = Infallible> + Clone {
    ping()
        .or(run(context_store.clone(), worker_context.clone()))
        .or(reload_config(context_store.clone(), worker_context.clone()))
        .or(reload_project(
            context_store.clone(),
            worker_context.clone(),
        ))
        .or(update_repo(context_store.clone(), worker_context.clone()))
        .recover(report_rejection)
}

pub fn ping() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("ping").and(warp::get()).map(|| StatusCode::OK)
}

pub fn run(
    context: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_validation())
        .and(warp::path!("call" / String / String))
        .and(warp::post())
        .and(with_context(context))
        .and(with_worker_context(worker_context))
        .and_then(handlers::call)
}

pub fn reload_config(
    context: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_validation())
        .and(warp::path!("reload"))
        .and(warp::post())
        .and(with_context(context))
        .and(with_worker_context(worker_context))
        .and_then(handlers::reload_config)
}

pub fn reload_project(
    context: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_validation())
        .and(warp::path!("reload" / String))
        .and(warp::post())
        .and(with_context(context))
        .and(with_worker_context(worker_context))
        .and_then(handlers::reload_project)
}

pub fn update_repo(
    context: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_validation())
        .and(warp::path!("update" / String))
        .and(warp::post())
        .and(with_context(context))
        .and(with_worker_context(worker_context))
        .and_then(handlers::update_repo)
}

pub fn with_context(
    context: ContextStore,
) -> impl Filter<Extract = (ContextStore,), Error = Infallible> + Clone {
    warp::any().map(move || context.clone())
}

pub fn with_worker_context(
    worker_context: Option<worker_lib::context::Context>,
) -> impl Filter<Extract = (Option<worker_lib::context::Context>,), Error = Infallible> + Clone {
    warp::any().map(move || worker_context.clone())
}

#[derive(Debug)]
pub enum Unauthorized {
    UnsupportedAuthorizationMethod(String),
    MethodIsNotSepcified,
    TokenIsNotSpecified,
    TokenIsUnauthorized,
    MissingAuthorizationHeader,
}

impl warp::reject::Reject for Unauthorized {}

pub fn with_validation() -> impl Filter<Extract = (Option<String>,), Error = Rejection> + Clone {
    warp::header::optional("Authorization").and_then(|auth: Option<String>| async move {
        if let Some(auth) = auth {
            let mut split = auth.split_whitespace();
            if let Some(method) = split.next() {
                if method != "Api-Key" {
                    Err(warp::reject::custom(
                        Unauthorized::UnsupportedAuthorizationMethod(method.to_string()),
                    ))
                } else {
                    if let Some(token) = split.next() {
                        Ok(Some(token.to_string()))
                    } else {
                        Err(warp::reject::custom(Unauthorized::TokenIsNotSpecified))
                    }
                }
            } else {
                Err(warp::reject::custom(Unauthorized::MethodIsNotSepcified))
            }
        } else {
            Ok(None)
        }
    })
}

pub async fn report_rejection(r: Rejection) -> Result<impl warp::Reply, Infallible> {
    if let Some(auth_error) = r.find::<Unauthorized>() {
        let message = match auth_error {
            Unauthorized::UnsupportedAuthorizationMethod(method) => {
                format!("Unsupported auth method {}", method)
            }
            Unauthorized::MethodIsNotSepcified => format!("Auth method is not specified"),
            Unauthorized::TokenIsNotSpecified => format!("Auth token is not specified"),
            Unauthorized::TokenIsUnauthorized => {
                format!("Specified token is unauthrized for this action")
            }
            Unauthorized::MissingAuthorizationHeader => format!("Authorization header is missing"),
        };
        Ok(warp::reply::with_status(
            warp::reply::json(&common::runner::ErrorResponse { message }),
            StatusCode::UNAUTHORIZED,
        ))
    } else {
        if r.is_not_found() {
            Ok(warp::reply::with_status(
                warp::reply::json(&common::runner::ErrorResponse {
                    message: "Not found".to_string(),
                }),
                StatusCode::NOT_FOUND,
            ))
        } else {
            Ok(warp::reply::with_status(
                warp::reply::json(&common::runner::ErrorResponse {
                    message: "Unknown error".to_string(),
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}
