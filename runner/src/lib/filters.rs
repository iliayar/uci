use std::{convert::Infallible, sync::Arc};
use warp::Filter;

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
    context: Context,
    worker_context: Option<worker_lib::context::Context>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let context_store = ContextStore::new(context);
    ping()
        .or(run(context_store.clone(), worker_context.clone()))
        .or(reload_config(context_store.clone()))
}

pub fn ping() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("ping").and(warp::get()).map(|| StatusCode::OK)
}

pub fn run(
    context: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!(String / "run_action" / String)
        .and(warp::post())
        .and(with_context(context))
        .and(with_worker_context(worker_context))
        .and_then(handlers::run)
}

pub fn reload_config(
    context: ContextStore,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("reload_config")
        .and(warp::post())
        .and(with_context(context))
        .and_then(handlers::reload_config)
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
