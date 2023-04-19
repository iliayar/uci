use std::path::PathBuf;

use hyper::Body;
use tokio_util::codec::{BytesCodec, FramedRead};

use tokio::fs::File;

use common::utils::tempfile;

use log::*;

pub async fn open_async_stream(filename: PathBuf) -> Result<Body, tokio::io::Error> {
    let file = File::open(filename).await?;
    let stream = FramedRead::new(file, BytesCodec::new());

    Ok(Body::wrap_stream(stream))
}

pub async fn create_temp_tar(directory: PathBuf) -> Result<tempfile::TempFile, tokio::io::Error> {
    let tempfile = tempfile::TempFile::empty().await;

    // TODO: Get rid of this sync
    info!("Creating temporary tar in {:?}", tempfile.path);
    let file = std::fs::File::create(&tempfile.path)?;

    info!("Writing tar");
    let mut tar = tar::Builder::new(file);
    tar.append_dir_all(".", directory)?;

    Ok(tempfile)
}
