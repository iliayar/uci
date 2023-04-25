use crate::{cli::*, execute};

pub async fn execute_pipeline(
    config: &crate::config::Config,
    command: PipelineCommand,
) -> Result<(), execute::ExecuteError> {
    match command {
        PipelineCommand::List {} => super::list::execute_pipelines_list(config).await?,
    }

    Ok(())
}
