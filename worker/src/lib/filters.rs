use std::convert::Infallible;
use warp::Filter;

use super::docker;
use super::handlers;
use warp::hyper::StatusCode;

pub fn runner(
    docker: docker::Docker,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    ping().or(run_filter(docker))
}

pub fn ping() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("ping").and(warp::get()).map(|| StatusCode::OK)
}

pub fn run_filter(
    docker: docker::Docker,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("run")
        .and(warp::post())
        .and(with_docker(docker))
        .and(warp::body::json())
        .and_then(handlers::run)
}

pub fn with_docker(
    docker: docker::Docker,
) -> impl Filter<Extract = (docker::Docker,), Error = Infallible> + Clone {
    warp::any().map(move || docker.clone())
}
