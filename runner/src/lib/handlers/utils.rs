use std::collections::HashSet;

use crate::lib::{
    config::{ActionEvent, ActionType},
    filters::CallContext,
};

use log::*;

pub struct ReloadResult {
    pub client_id: Option<String>,
    pub done: bool,
    pub missing_repos: Option<HashSet<String>>,
}

pub async fn reload_config_impl(call_context: CallContext) -> Result<ReloadResult, anyhow::Error> {
    Ok(reload_impl(call_context, ActionEvent::ConfigReloaded).await?)
}

pub async fn reload_project_impl(
    call_context: CallContext,
    project_id: String,
) -> Result<ReloadResult, anyhow::Error> {
    Ok(reload_impl(call_context, ActionEvent::ProjectReloaded { project_id }).await?)
}

async fn reload_impl(
    mut call_context: CallContext,
    event: ActionEvent,
) -> Result<ReloadResult, anyhow::Error> {
    let config = call_context.store.context().preload_config().await?;
    let missing_repos = config.get_missing_repos().await?;

    if missing_repos.is_empty() {
        call_context.store.context().load_config(config).await?;
        tokio::spawn(trigger_projects_impl(call_context, event));
        Ok(ReloadResult {
            client_id: None,
            done: true,
            missing_repos: None,
        })
    } else {
        let client_id = call_context.init_ws().await;
        tokio::spawn(clone_repos_trigger_projects_impl(
            call_context,
            config,
            event,
        ));
        Ok(ReloadResult {
            client_id: Some(client_id),
            done: false,
            missing_repos: Some(missing_repos),
        })
    }
}

async fn clone_repos_trigger_projects_impl(
    call_context: CallContext,
    config: crate::lib::config::ConfigPreload,
    event: ActionEvent,
) {
    match config.clone_missing_repos().await {
        Err(err) => {
            error!("Failed to clone repos: {}", err);
            return;
        }
        Ok(_) => {}
    }
    call_context
        .send(common::runner::ReloadConfigMessage::ReposCloned)
        .await;
    match call_context.store.context().load_config(config).await {
        Err(err) => {
            error!("Failed to load config: {}", err);
            call_context
                .send(common::runner::ReloadConfigMessage::ConfigReloadedError(
                    err.to_string(),
                ))
                .await;
            return;
        }
        Ok(_) => {
            call_context
                .send(common::runner::ReloadConfigMessage::ConfigReloaded)
                .await;
        }
    }
    trigger_projects_impl(call_context, event).await;
}

pub async fn trigger_projects_impl(call_context: CallContext, event: ActionEvent) {
    match trigger_projects_impl_result(call_context, event).await {
        Result::Err(err) => {
            error!("Failed to match actions: {}", err);
        }
        Result::Ok(_) => {}
    }
}

pub async fn trigger_projects_impl_result(
    call_context: CallContext,
    event: ActionEvent,
) -> Result<(), anyhow::Error> {
    let mut matched = call_context.get_actions(event).await?;

    if matched.reload_config {
        if call_context
            .check_allowed::<&str>(None, ActionType::Write)
            .await
        {
            return Err(anyhow::anyhow!(
                "Reloading config is not allowed, do nothing"
            ));
        }
    }

    // FIXME: Do it separately?
    if matched.reload_config || !matched.reload_projects.is_empty() {
        let config = call_context.store.context().preload_config().await?;
        config.clone_missing_repos().await?;
        call_context.store.context().load_config(config).await?;
        matched.merge(
            call_context
                .get_actions(ActionEvent::ConfigReloaded)
                .await?,
        );
    }

    let mut new_matcheds = Vec::new();
    for project_id in matched.reload_projects.iter() {
        if !call_context
            .check_allowed(Some(&project_id), ActionType::Write)
            .await
        {
            warn!("Not allowed to reload project {}, do nothing", project_id);
            continue;
        }

        new_matcheds.push(
            call_context
                .get_actions(ActionEvent::ProjectReloaded {
                    project_id: project_id.clone(),
                })
                .await?,
        );
    }
    for new_matched in new_matcheds.into_iter() {
        matched.merge(new_matched);
    }

    let execution_context = call_context.to_execution_context().await;
    execution_context
        .config()
        .run_project_actions(&execution_context, matched)
        .await?;

    Ok(())
}
