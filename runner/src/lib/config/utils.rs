use std::{collections::HashMap, path::PathBuf};

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

pub fn prepare_links(
    project_id: &str,
    config: &super::ServiceConfig,
    unprepared_links: &HashMap<String, String>,
) -> HashMap<String, String> {
    let substitutions: HashMap<String, PathBuf> = HashMap::from([
        (String::from("repos"), config.repos_path.clone()),
        (String::from("data"), config.data_path.join(project_id)),
    ]);

    let mut links = HashMap::new();
    for (link, path) in unprepared_links.into_iter() {
        let new_path = substitute_path(&substitutions, path);
        links.insert(link.clone(), new_path);
    }

    links
}

fn substitute_path(substitutions: &HashMap<String, PathBuf>, path: &str) -> String {
    for (var, subst) in substitutions {
        if let Some(rel_path) = path.strip_prefix(&format!("${}/", var)) {
            return subst.join(rel_path).to_string_lossy().to_string();
        }
    }

    path.to_string()
}
