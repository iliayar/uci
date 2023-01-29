use std::convert::Infallible;

use warp::hyper::StatusCode;

pub async fn ping() -> Result<impl warp::Reply, Infallible> {
    log::debug!("Handle /ping");

    Ok(StatusCode::OK)
}
