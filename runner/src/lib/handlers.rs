use std::convert::Infallible;

use log::*;
use warp::hyper::StatusCode;

pub async fn hook(project_id: String, action: String) -> Result<impl warp::Reply, Infallible> {
    info!("Running hook {} for project {}", action, project_id);
    // TODO
    Ok(StatusCode::OK)
}
