use crate::{cli::*, execute};

pub async fn execute_pipeline(
    config: &crate::config::Config,
    command: PipelineCommand,
) -> Result<(), execute::ExecuteError> {
    match command {
        PipelineCommand::List { project } => {
            super::list::execute_pipelines_list(config, project).await?
        }
    }

    Ok(())
}
