use std::convert::Infallible;
use warp::Filter;

use warp::hyper::StatusCode;

use bollard::Docker;

use super::handlers;

pub fn runner(docker: handlers::docker::Docker) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    ping().or(docker_filters(docker))
}

pub fn ping() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("ping").and(warp::get()).map(|| StatusCode::OK)
}

pub fn docker_filters(
    docker: handlers::docker::Docker,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let start = warp::path("run")
        .and(warp::post())
        .and(with_docker(docker))
        .and(warp::body::json())
        .and_then(handlers::docker::run);

    warp::path("docker").and(start)
}

pub fn with_docker(
    docker: handlers::docker::Docker,
) -> impl Filter<Extract = (handlers::docker::Docker,), Error = Infallible> + Clone {
    warp::any().map(move || docker.clone())
}
