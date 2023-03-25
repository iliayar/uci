use std::convert::Infallible;
use warp::Filter;

use super::docker;
use super::handlers;
use super::context::Context;
use warp::hyper::StatusCode;

pub fn runner(
    context: Context,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    ping().or(run_filter(context))
}

pub fn ping() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("ping").and(warp::get()).map(|| StatusCode::OK)
}

pub fn run_filter(
    context: Context,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("run")
        .and(warp::post())
        .and(with_context(context))
        .and(warp::body::json())
        .and_then(handlers::run)
}

pub fn with_context(
    context: Context,
) -> impl Filter<Extract = (Context,), Error = Infallible> + Clone {
    warp::any().map(move || context.clone())
}
