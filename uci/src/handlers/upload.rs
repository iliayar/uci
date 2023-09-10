use futures::StreamExt;
use runner_lib::{call_context, config};
use tokio::io::AsyncWriteExt;

use crate::filters::{with_call_context, AuthRejection, InternalServerError};

use reqwest::StatusCode;
use warp::{multipart::FormData, Filter};

use anyhow::anyhow;
use log::*;

pub fn filter(
    context: call_context::Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(warp::path!("upload"))
        .and(with_call_context(context))
        .and(warp::multipart::form())
        .and(warp::post())
        .and_then(upload)
}

async fn upload(
    call_context: call_context::CallContext,
    form: FormData,
) -> Result<impl warp::Reply, warp::Rejection> {
    if !call_context
        .check_permissions(None, config::permissions::ActionType::Write)
        .await
    {
        return Err(warp::reject::custom(AuthRejection::TokenIsUnauthorized));
    }

    match upload_impl(call_context.artifacts, form).await {
        Ok(resp) => Ok(warp::reply::with_status(
            warp::reply::json(&resp),
            StatusCode::OK,
        )),
        Err(err) => Err(warp::reject::custom(InternalServerError::Error(
            err.to_string(),
        ))),
    }
}

async fn upload_impl(
    artifacts: call_context::ArtifactsStorage,
    mut form: FormData,
) -> Result<models::UploadResponse, anyhow::Error> {
    let (artifact, filename) = artifacts.create().await;
    info!("Uploading artifact {}", artifact);

    while let Some(part) = form.next().await {
        let part = part?;
        if part.name() == "file" {
            let mut file = tokio::fs::File::create(&filename).await?;
            let mut chunks = part.stream();
            while let Some(chunk) = chunks.next().await {
                let mut chunk = chunk?;
                file.write_all_buf(&mut chunk).await?;
            }

            return Ok(models::UploadResponse { artifact });
        }
    }

    Err(anyhow!("No file in form data"))
}
