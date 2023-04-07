pub mod bind;
pub mod caddy;
pub mod project;

#[derive(Debug, thiserror::Error)]
pub enum CodegenError {
    #[error("Codegen failed due to io error: {0}")]
    IOError(#[from] tokio::io::Error),
}
