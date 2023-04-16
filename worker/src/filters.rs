use std::{convert::Infallible, sync::Arc};
use common::state::State;
use warp::Filter;

use super::handlers;
use warp::hyper::StatusCode;

#[derive(Clone)]
pub struct Deps {
    pub state: Arc<State<'static>>,
}

pub fn runner(
    deps: Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    ping().or(run_filter(deps))
}

pub fn ping() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("ping").and(warp::get()).map(|| StatusCode::OK)
}

pub fn run_filter(
    deps: Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("run")
        .and(warp::post())
        .and(with_context(deps))
        .and(warp::body::json())
        .and_then(handlers::run)
}

pub fn with_context(
    deps: Deps,
) -> impl Filter<Extract = (Deps,), Error = Infallible> + Clone {
    warp::any().map(move || deps.clone())
}
