use std::convert::Infallible;

use log::*;
use warp::hyper::StatusCode;

use super::{context::Context, filters::ContextStore};

pub async fn hook(
    project_id: String,
    action: String,
    store: ContextStore,
) -> Result<impl warp::Reply, Infallible> {
    info!("Running hook {} for project {}", action, project_id);
    // TODO
    Ok(StatusCode::OK)
}
