use super::error::TaskError;
use crate::lib::{docker, utils};
use bollard::image::{BuildImageOptions, CreateImageOptions};
use common::{
    BuildImageArchiveConfig, BuildImageConfig, BuildImagePathConfig, BuildImagePullConfig,
    BuildImageSource,
};
use futures::{AsyncWrite, StreamExt};
use log::*;
use tokio_util::codec::{BytesCodec, FramedRead};
use warp::hyper::Body;

use tar;

pub async fn docker_build(
    docker: &docker::Docker,
    config: BuildImageConfig,
) -> Result<(), TaskError> {
    match config.source {
        BuildImageSource::Path(config) => docker_build_from_path(docker, config).await?,
        BuildImageSource::Archive(config) => docker_build_from_archive(docker, config).await?,
        BuildImageSource::Pull(config) => docker_build_pull(docker, config).await?,
    }

    Ok(())
}

async fn docker_build_from_path(
    docker: &docker::Docker,
    config: BuildImagePathConfig,
) -> Result<(), TaskError> {
    let filename = utils::tempfile::get_temp_filename().await;

    // TODO: Get rid of this sync
    info!("Creating temporary tar in {:?}", filename);
    let file = std::fs::File::create(filename.clone())?;

    info!("Writing tar");
    let mut tar = tar::Builder::new(file);
    tar.append_dir_all(".", config.path)?;

    drop(tar);

    docker_build_from_archive(
        docker,
        BuildImageArchiveConfig {
            tar_path: filename.to_str().unwrap().to_string(),
            dockerfile: config.dockerfile,
            tag: config.tag,
        },
    )
    .await?;

    info!("Removing tar at {:?}", filename);
    tokio::fs::remove_file(filename).await?;

    Ok(())
}

async fn docker_build_from_archive(
    docker: &docker::Docker,
    config: BuildImageArchiveConfig,
) -> Result<(), TaskError> {
    let archive = tokio::fs::File::open(config.tar_path).await?;
    let stream = FramedRead::new(archive, BytesCodec::new());
    let body = Body::wrap_stream(stream);

    // TODO: Remove clone
    let mut results = docker.con.build_image(
        BuildImageOptions {
            dockerfile: config.dockerfile.unwrap_or(String::from("Dockerfile")),
            t: config.tag.clone(),
            ..Default::default()
        },
        None,
        Some(body),
    );

    // TODO: Generalize logging this
    while let Some(result) = results.next().await {
        let result = result?;
        let progress = result.stream.unwrap_or(String::from("<unknown>"));
        info!("Builing image {}: {}", config.tag, progress);
    }
    info!("Building image {} done", config.tag);

    Ok(())
}

async fn docker_build_pull(
    docker: &docker::Docker,
    config: BuildImagePullConfig,
) -> Result<(), TaskError> {
    // TODO: Remove clone
    let mut results = docker.con.create_image(
        Some(CreateImageOptions {
            from_image: config.image.clone(),
            tag: config.tag.unwrap_or(String::from("latest")),
            ..Default::default()
        }),
        None,
        None,
    );

    // TODO: Generalize logging this
    while let Some(result) = results.next().await {
        let result = result?;
        let status = result.status.unwrap_or(String::from("<unknown>"));
        info!("Pulling image {}: {}", config.image, status);
    }
    info!("Pulling image {} done", config.image);

    Ok(())
}
