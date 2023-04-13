// impl Config {
    // pub async fn preload(
    //     configs_root: PathBuf,
    //     env: String,
    // ) -> Result<ConfigPreload, super::LoadConfigError> {
    //     info!("Preloading config");

    //     let mut load_context = super::LoadContext::default();
    //     load_context.set_configs_root(&configs_root);
    //     load_context.set_env(&env);

    //     let service_config = super::ServiceConfig::load(&load_context).await?;
    //     load_context.set_config(&service_config);

    //     let repos = super::Repos::load(&load_context).await?;
    //     load_context.set_repos(&repos);

    //     Ok(ConfigPreload {
    //         service_config,
    //         repos,
    //         configs_root,
    //         env,
    //     })
    // }

//     pub async fn get_projects_actions(
//         &self,
//         event: ActionEvent,
//     ) -> Result<super::MatchedActions, super::ExecutionError> {
//         let event = match event {
//             ActionEvent::DirectCall {
//                 project_id,
//                 trigger_id,
//             } => super::Event::Call {
//                 project_id,
//                 trigger_id,
//             },
//             ActionEvent::ConfigReloaded => super::Event::ConfigReloaded,
//             ActionEvent::ProjectReloaded { project_id } => {
//                 super::Event::ProjectReloaded { project_id }
//             }
//             ActionEvent::UpdateRepos { repos } => {
//                 let diffs = self
//                     .repos
//                     .pull_all(&self.service_config, Some(repos.into_iter().collect()))
//                     .await?;
//                 super::Event::RepoUpdate { diffs }
//             }
//         };

//         self.projects.get_matched(&event).await
//     }

//     pub async fn run_project_actions(
//         &self,
//         execution_context: &ExecutionContext,
//         matched: super::MatchedActions,
//     ) -> Result<(), super::ExecutionError> {
//         info!("Running actions: {:#?}", matched);
//         self.projects
//             .run_matched(execution_context, matched)
//             .await?;

//         Ok(())
//     }
// }
