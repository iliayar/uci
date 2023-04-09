#[derive(serde::Serialize, serde::Deserialize)]
pub struct EmptyResponse {}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ErrorResponse {
    pub message: String,
}
