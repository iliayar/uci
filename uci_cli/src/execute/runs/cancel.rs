use crate::execute;

use crate::utils::WithSpinner;

use log::*;
use termion::{color, style};

pub async fn execute_runs_cancel(
    config: &crate::config::Config,
    pipeline: Option<String>,
    run: Option<String>,
) -> Result<(), execute::ExecuteError> {
    let project = config.get_project().await;
    debug!("Executing cancel run command");

    let (pipeline, run) = match (pipeline, run) {
        (Some(pipeline), Some(run)) => (pipeline, run),
        (pipeline, run) => {
            let run =
                crate::prompts::promp_run(config, Some(project.clone()), run, pipeline).await?;
            (run.pipeline_id, run.run_id)
        }
    };

    let body = models::RunsCancelRequestBody {
        project,
        pipeline,
        run,
    };
    let response: models::EmptyResponse = async {
        let response = crate::runner::post_body(config, "/runs/cancel", &body)?
            .send()
            .await;
        crate::runner::json(response).await
    }
    .with_spinner("Canceling run")
    .await?;

    println!("{}Run canceled{}", color::Fg(color::Green), style::Reset);

    Ok(())
}
