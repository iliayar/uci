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
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let context_store = ContextStore::new(context);
    ping().or(hook(context_store.clone()))
}

pub fn ping() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("ping").and(warp::get()).map(|| StatusCode::OK)
}

pub fn hook(
    context: ContextStore,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("hook" / String / String)
        .and(warp::post())
        .and(with_context(context))
        .and_then(handlers::hook)
}

pub fn with_context(
    context: ContextStore,
) -> impl Filter<Extract = (ContextStore,), Error = Infallible> + Clone {
    warp::any().map(move || context.clone())
}
