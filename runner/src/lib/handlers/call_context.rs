use crate::lib::{config, filters::ContextPtr};

use log::*;

pub struct CallContext<PM: config::ProjectsManager> {
    pub token: Option<String>,
    pub check_permisions: bool,
    pub context: ContextPtr<PM>,
}

impl<PM: config::ProjectsManager> CallContext<PM> {
    pub fn for_handler(token: Option<String>, context: ContextPtr<PM>) -> CallContext<PM> {
        CallContext {
            token,
            check_permisions: true,
            context,
        }
    }

    pub async fn update_repo(&self, project_id: &str, repo_id: &str) -> Result<(), anyhow::Error> {
        self.context.update_repo(project_id, repo_id).await
    }

    pub async fn reload_config(&self) -> Result<(), anyhow::Error> {
        self.context.reload_config().await
    }

    pub async fn list_projects(&self) -> Result<Vec<config::ProjectInfo>, anyhow::Error> {
        self.context.list_projects().await
    }

    pub async fn call_trigger(&self, project_id: &str, trigger_id: &str) -> Result<(), anyhow::Error> {
	self.context.call_trigger(project_id, trigger_id).await
    }

    pub async fn check_permissions(
        &self,
        project_id: Option<&str>,
        action: config::ActionType,
    ) -> bool {
        if !self.check_permisions {
            return true;
        }
        if let Some(project_id) = project_id {
            match self.context.get_project_info(project_id).await {
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
}
