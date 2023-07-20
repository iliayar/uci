use models;

use reqwest::header;

use anyhow::anyhow;
use log::*;

pub trait RunnerClientConfig {
    fn runner_url(&self) -> Option<&str>;
    fn ws_runner_url(&self) -> Option<&str>;
    fn token(&self) -> Option<&str>;
}

fn call_runner<C: RunnerClientConfig>(config: &C) -> Result<reqwest::Client, anyhow::Error> {
    let mut headers = header::HeaderMap::new();

    if let Some(token) = config.token().as_ref() {
        let mut auth_value = header::HeaderValue::from_str(&format!("Api-Key {}", token))?;
        auth_value.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, auth_value);
    }

    Ok(reqwest::Client::builder()
        .default_headers(headers)
        .build()?)
}

pub fn post<C: RunnerClientConfig, S: AsRef<str>>(
    config: &C,
    path: S,
) -> Result<reqwest::RequestBuilder, anyhow::Error> {
    let runner_url = config
        .runner_url()
        .ok_or_else(|| anyhow!("runner_url is not set"))?;
    Ok(call_runner(config)?.post(format!("{}{}", runner_url, path.as_ref())))
}

pub fn post_body<C: RunnerClientConfig, S: AsRef<str>, T: serde::Serialize>(
    config: &C,
    path: S,
    body: &T,
) -> Result<reqwest::RequestBuilder, anyhow::Error> {
    let runner_url = config
        .runner_url()
        .ok_or_else(|| anyhow!("runner_url is not set"))?;
    Ok(call_runner(config)?
        .post(format!("{}{}", runner_url, path.as_ref()))
        .json(body))
}

pub fn get<C: RunnerClientConfig, S: AsRef<str>>(
    config: &C,
    path: S,
) -> Result<reqwest::RequestBuilder, anyhow::Error> {
    let runner_url = config
        .runner_url()
        .ok_or_else(|| anyhow!("runner_url is not set"))?;
    Ok(call_runner(config)?.get(format!("{}{}", runner_url, path.as_ref())))
}

pub fn get_query<C: RunnerClientConfig, S: AsRef<str>, T: serde::Serialize>(
    config: &C,
    path: S,
    query: &T,
) -> Result<reqwest::RequestBuilder, anyhow::Error> {
    let runner_url = config
        .runner_url()
        .ok_or_else(|| anyhow!("runner_url is not set"))?;
    Ok(call_runner(config)?
        .get(format!("{}{}", runner_url, path.as_ref()))
        .query(query))
}

pub fn get_query_body<
    C: RunnerClientConfig,
    S: AsRef<str>,
    T: serde::Serialize,
    E: serde::Serialize,
>(
    config: &C,
    path: S,
    query: &T,
    body: &E,
) -> Result<reqwest::RequestBuilder, anyhow::Error> {
    let runner_url = config
        .runner_url()
        .ok_or_else(|| anyhow!("runner_url is not set"))?;
    Ok(call_runner(config)?
        .get(format!("{}{}", runner_url, path.as_ref()))
        .query(query)
        .json(body))
}

pub async fn json<T: for<'a> serde::Deserialize<'a>>(
    response: reqwest::Result<reqwest::Response>,
) -> Result<T, anyhow::Error> {
    match response {
        Ok(response) => {
            info!("Get reponse with status {:?}", response.status());
            if response.status().is_success() {
                Ok(response.json().await.map_err(Into::<anyhow::Error>::into)?)
            } else {
                let status = response.status();
                let text = response.text().await.map_err(Into::<anyhow::Error>::into)?;
                match serde_json::from_str::<models::ErrorResponse>(&text) {
                    Ok(error_response) => Err(anyhow!("{}", error_response.message)),
                    Err(err) => Err(anyhow!(
                        "Failed to parse response as json ({}). Got {}: {}",
                        err,
                        status,
                        text
                    )),
                }
            }
        }
        Err(err) => Err(Into::<anyhow::Error>::into(err).into()),
    }
}

pub mod api {
    use crate::RunnerClientConfig;

    pub async fn list_services<C: RunnerClientConfig>(
        config: &C,
        project_id: String,
    ) -> Result<models::ServicesListResponse, anyhow::Error> {
        let query = models::ListServicesQuery { project_id };
        let response = super::get_query(config, "/projects/services/list", &query)?
            .send()
            .await;
        super::json(response).await
    }

    pub async fn list_runs<C: RunnerClientConfig>(
        config: &C,
        project_id: Option<String>,
        pipeline_id: Option<String>,
    ) -> Result<models::ListRunsResponse, anyhow::Error> {
        let query = models::ListRunsRequestQuery {
            project_id,
            pipeline_id,
        };
        let response = super::get_query(config, "/runs/list", &query)?.send().await;
        super::json(response).await
    }

    pub async fn list_actions<C: RunnerClientConfig>(
        config: &C,
        project_id: String,
    ) -> Result<models::ActionsListResponse, anyhow::Error> {
        let query = models::ListActionsQuery { project_id };
        let response = super::get_query(config, "/projects/actions/list", &query)?
            .send()
            .await;
        super::json(response).await
    }

    pub async fn projects_list<C: RunnerClientConfig>(
        config: &C,
    ) -> Result<models::ProjectsListResponse, anyhow::Error> {
        let response = super::get(config, "/projects/list")?.send().await;
        super::json(response).await
    }

    pub async fn repos_list<C: RunnerClientConfig>(
        config: &C,
        project_id: String,
    ) -> Result<models::ReposListResponse, anyhow::Error> {
        let query = models::ListReposQuery { project_id };
        let response = super::get_query(config, "/projects/repos/list", &query)?
            .send()
            .await;
        super::json(response).await
    }

    pub async fn upload<C: RunnerClientConfig>(
        config: &C,
        data: Vec<u8>,
    ) -> Result<models::UploadResponse, anyhow::Error> {
        let file_part = reqwest::multipart::Part::bytes(data);
        let form = reqwest::multipart::Form::new().part("file", file_part);
        let response = super::post(config, "/upload")?.multipart(form).send().await;
        super::json(response).await
    }

    pub async fn reload_config<C: RunnerClientConfig>(
        config: &C,
    ) -> Result<models::EmptyResponse, anyhow::Error> {
        let response = super::post(config, "/reload")?.send().await;
        super::json(response).await
    }

    pub async fn action_call<C: RunnerClientConfig>(
        config: &C,
        request: &models::CallRequest,
    ) -> Result<models::ContinueReponse, anyhow::Error> {
        let response = super::post_body(config, "/call", request)?.send().await;
        super::json(response).await
    }

    pub async fn run_logs<C: RunnerClientConfig>(
        config: &C,
        query: &models::RunsLogsRequestQuery,
    ) -> Result<models::ContinueReponse, anyhow::Error> {
        let response = super::get_query(config, "/runs/logs", query)?.send().await;
        super::json(response).await
    }
}
