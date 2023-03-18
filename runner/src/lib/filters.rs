use std::convert::Infallible;
use warp::Filter;

use super::handlers;
use warp::hyper::StatusCode;

pub fn runner(
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    ping()
}

pub fn ping() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("ping").and(warp::get()).map(|| StatusCode::OK)
}
