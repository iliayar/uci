use std::{collections::HashMap, sync::Arc};

use crate::config;
use crate::context::Context;

use common::{
    run_context::{RunContext, WsClientReciever},
    state::State,
};
use log::*;
use tokio::sync::Mutex;

pub type Runs = Arc<Mutex<HashMap<String, Arc<RunContext>>>>;
pub type ContextPtr<PM> = Arc<Context<PM>>;

pub struct Deps<PM: config::ProjectsManager> {
    pub context: Arc<Context<PM>>,
    pub runs: Runs,
    pub state: Arc<State<'static>>,
}

impl<PM: config::ProjectsManager> Clone for Deps<PM> {
    fn clone(&self) -> Self {
        Self {
            context: self.context.clone(),
            runs: self.runs.clone(),
            state: self.state.clone(),
        }
    }
}

pub struct CallContext<PM: config::ProjectsManager> {
    pub token: Option<String>,
    check_permisions: bool,
    pub context: ContextPtr<PM>,
    pub runs: Arc<Mutex<HashMap<String, Arc<RunContext>>>>,
    pub run_context: Option<Arc<RunContext>>,
    pub state: Arc<State<'static>>,
}

impl<PM: config::ProjectsManager> CallContext<PM> {
    pub fn for_handler(token: Option<String>, deps: Deps<PM>) -> CallContext<PM> {
        CallContext {
            token,
            context: deps.context,
            runs: deps.runs,
            check_permisions: true,
            run_context: None,
            state: deps.state,
        }
    }

    pub async fn run_service_action(
        &self,
        project: &str,
        service: &str,
        action: config::ServiceAction,
    ) -> Result<(), anyhow::Error> {
        let mut state = self.state.as_ref().clone();
        if let Some(run_context) = self.run_context.as_ref() {
            state.set(run_context.as_ref());
        }
        self.context
            .run_service_action(&state, project, service, action)
            .await
    }

    pub async fn update_repo(&self, project_id: &str, repo_id: &str) -> Result<(), anyhow::Error> {
        let mut state = self.state.as_ref().clone();
        if let Some(run_context) = self.run_context.as_ref() {
            state.set(run_context.as_ref());
        }
        self.context.update_repo(&state, project_id, repo_id).await
    }

    pub async fn reload_config(&self) -> Result<(), anyhow::Error> {
        self.context.reload_config().await
    }

    pub async fn list_projects(&self) -> Result<Vec<config::ProjectInfo>, anyhow::Error> {
        let state = self.state.as_ref().clone();
        self.context.list_projects(&state).await
    }

    pub async fn get_project(&self, project_id: &str) -> Result<config::Project, anyhow::Error> {
        let mut state = self.state.as_ref().clone();

        let run_context = RunContext::empty();
        state.set(&run_context);

        self.context.get_project(&state, project_id).await
    }

    pub async fn call_trigger(
        &self,
        project_id: &str,
        trigger_id: &str,
    ) -> Result<(), anyhow::Error> {
        let mut state = self.state.as_ref().clone();
        if let Some(run_context) = self.run_context.as_ref() {
            state.set(run_context.as_ref());
        }
        self.context
            .call_trigger(&state, project_id, trigger_id)
            .await
    }

    pub async fn check_permissions(
        &self,
        project_id: Option<&str>,
        action: config::ActionType,
    ) -> bool {
        let state = self.state.as_ref().clone();
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
}
