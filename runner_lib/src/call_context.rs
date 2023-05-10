use std::{
    collections::{HashMap, LinkedList},
    path::PathBuf,
    sync::Arc,
};

use crate::config;
use crate::context::Context;

use common::{
    run_context::{RunContext, WsClientReciever},
    state::State,
};
use log::*;
use tokio::sync::Mutex;

pub type Runs = Arc<Mutex<HashMap<String, Arc<RunContext>>>>;
pub type ContextPtr = Arc<Context>;

#[derive(Clone)]
pub struct ArtifactsStorage {
    path: PathBuf,
    queue: Arc<Mutex<LinkedList<String>>>,
    queue_limit: usize,
}

impl ArtifactsStorage {
    pub async fn new(path: PathBuf, queue_limit: usize) -> Result<Self, anyhow::Error> {
        tokio::fs::remove_dir_all(&path).await.ok();
        tokio::fs::create_dir_all(&path).await?;

        assert!(
            queue_limit > 0,
            "Artifacts queue limit must be greater than zero"
        );

        Ok(Self {
            path,
            queue_limit,
            queue: Arc::new(Mutex::new(LinkedList::default())),
        })
    }

    pub async fn create(&self) -> (String, PathBuf) {
        let mut queue = self.queue.lock().await;
        if queue.len() > self.queue_limit {
            let id = queue.pop_front().unwrap();
        }

        let id = uuid::Uuid::new_v4().to_string();
        queue.push_back(id.clone());

        let path = self.path.join(&id);

        (id, path)
    }

    pub fn get_path(&self, id: impl AsRef<str>) -> PathBuf {
        // NOTE: Maybe mark this id as used and delete used id's first
        // when reaching the limit
        self.path.join(id.as_ref())
    }
}

#[derive(Clone)]
pub struct Deps {
    pub context: Arc<Context>,
    pub runs: Runs,
    pub state: Arc<State<'static>>,
    pub artifacts: ArtifactsStorage,
}

pub struct CallContext {
    pub token: Option<String>,
    check_permisions: bool,
    pub context: ContextPtr,
    pub runs: Arc<Mutex<HashMap<String, Arc<RunContext>>>>,
    pub run_context: Option<Arc<RunContext>>,
    pub state: Arc<State<'static>>,
    pub artifacts: ArtifactsStorage,
}

impl CallContext {
    pub async fn with_state<'a, R, F, Fut>(&'a self, f: F) -> R
    where
        Fut: futures::Future<Output = R>,
        F: FnOnce(State<'a>) -> Fut,
    {
        let mut state = self.state.as_ref().clone();
        if let Some(run_context) = self.run_context.as_ref() {
            state.set(run_context.as_ref());
        }
        f(state).await
    }

    pub fn for_handler(token: Option<String>, deps: Deps) -> CallContext {
        CallContext {
            token,
            context: deps.context,
            runs: deps.runs,
            check_permisions: true,
            run_context: Some(Arc::new(RunContext::empty())),
            state: deps.state,
            artifacts: deps.artifacts.clone(),
        }
    }

    pub async fn run_services_actions(
        &self,
        project: &str,
        services: Vec<String>,
        action: config::ServiceAction,
    ) -> Result<(), anyhow::Error> {
        self.with_state(|state| async move {
            self.context
                .run_services_actions(&state, project, services, action)
                .await
        })
        .await
    }

    pub async fn run_service_action(
        &self,
        project: &str,
        service: &str,
        action: config::ServiceAction,
    ) -> Result<(), anyhow::Error> {
        self.run_services_actions(project, vec![service.to_string()], action)
            .await
    }

    pub async fn update_repo(
        &self,
        project_id: &str,
        repo_id: &str,
        artifact: Option<PathBuf>,
        dry_run: bool,
	update_only: bool,
    ) -> Result<(), anyhow::Error> {
        self.with_state(|state| async move {
            let mut state = state.clone();
            state.set_named("dry_run", &dry_run);
            state.set_named("update_only", &update_only);
            self.context
                .update_repo(&state, project_id, repo_id, artifact)
                .await
        })
        .await
    }

    pub async fn reload_config(&self) -> Result<(), anyhow::Error> {
        self.context.reload_config().await
    }

    pub async fn list_projects(&self) -> Result<Vec<config::ProjectInfo>, anyhow::Error> {
        self.with_state(|state| async move { self.context.list_projects(&state).await })
            .await
    }

    pub async fn get_project(&self, project_id: &str) -> Result<config::Project, anyhow::Error> {
        self.with_state(|state| async move { self.context.get_project(&state, project_id).await })
            .await
    }

    pub async fn get_project_info(
        &self,
        project_id: &str,
    ) -> Result<config::ProjectInfo, anyhow::Error> {
        self.with_state(
            |state| async move { self.context.get_project_info(&state, project_id).await },
        )
        .await
    }

    pub async fn call_trigger(
        &self,
        project_id: &str,
        trigger_id: &str,
        dry_run: bool,
    ) -> Result<(), anyhow::Error> {
        self.with_state(|state| async move {
            let mut state = state.clone();
            state.set_named("dry_run", &dry_run);
            self.context
                .call_trigger(&state, project_id, trigger_id)
                .await
        })
        .await
    }

    pub async fn check_permissions(
        &self,
        project_id: Option<&str>,
        action: config::ActionType,
    ) -> bool {
        self.with_state(|state| async move {
            if !self.check_permisions {
                return true;
            }
            if let Some(project_id) = project_id {
                match self.context.get_project_info(&state, project_id).await {
                    Ok(project_info) => project_info.check_allowed(self.token.as_ref(), action),
                    Err(err) => {
                        error!(
                            "Failed to check permissions, cannot get project info: {}",
                            err
                        );
                        false
                    }
                }
            } else {
                self.context
                    .config()
                    .await
                    .check_allowed(self.token.as_ref(), action)
            }
        })
        .await
    }

    pub async fn init_run(&mut self) -> String {
        self.init_run_impl(RunContext::new()).await
    }

    pub async fn init_run_buffered(&mut self) -> String {
        self.init_run_impl(RunContext::new_buffered()).await
    }

    async fn init_run_impl(&mut self, run_context: RunContext) -> String {
        let run_context = Arc::new(run_context);
        self.runs
            .lock()
            .await
            .insert(run_context.id.clone(), run_context.clone());
        self.run_context = Some(run_context.clone());
        debug!("New ws run registerd: {}", run_context.id);
        run_context.id.clone()
    }

    pub async fn make_out_channel(&self, run_id: String) -> Option<WsClientReciever> {
        if let Some(run_context) = self.runs.lock().await.get(&run_id) {
            Some(run_context.make_client_receiver().await)
        } else {
            error!("Trying get not existing run {}", run_id);
            None
        }
    }

    pub async fn finish_run(&mut self) {
        if let Some(run_context) = self.run_context.take() {
            self.runs.lock().await.remove(&run_context.id);
        }
    }

    pub async fn wait_for_clients(&self, time_limit: std::time::Duration) -> bool {
        if let Some(run_context) = self.run_context.as_ref() {
            run_context.wait_for_client(time_limit).await
        } else {
            false
        }
    }
}
