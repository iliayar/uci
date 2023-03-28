use std::path::PathBuf;

pub async fn load_file<R, T>(path: PathBuf) -> Result<T, super::LoadConfigError>
where
    R: for<'a> serde::Deserialize<'a>,
    T: TryFrom<R>,
    <T as TryFrom<R>>::Error: Into<super::LoadConfigError>,
{
    let content = tokio::fs::read_to_string(path).await?;
    serde_yaml::from_str::<R>(&content)?
        .try_into()
        .map_err(Into::into)
}
