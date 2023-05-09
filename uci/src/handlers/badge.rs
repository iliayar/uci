use std::sync::Arc;

use serde::{Deserialize, Serialize};

use runner_lib::{call_context, config};

use crate::filters::{with_call_context, AuthRejection, InternalServerError};

use warp::Filter;

use anyhow::anyhow;
use log::*;

pub fn filter(
    deps: call_context::Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    filter_ghlike(deps)
}

#[derive(Serialize, Deserialize)]
struct QueryParams {
    project_id: String,
    pipeline_id: String,
    job_id: String,
}

pub fn filter_ghlike(
    deps: call_context::Deps,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_call_context(deps))
        .and(warp::path!("badge" / "ghlike"))
        .and(warp::query::<QueryParams>())
        .and(warp::get())
        .and_then(ghlike_badge)
}

async fn ghlike_badge(
    call_context: call_context::CallContext,
    QueryParams {
        project_id,
        pipeline_id,
        job_id,
    }: QueryParams,
) -> Result<impl warp::Reply, warp::Rejection> {
    if !call_context
        .check_permissions(Some(&project_id), config::ActionType::Read)
        .await
    {
        return Err(warp::reject::custom(AuthRejection::TokenIsUnauthorized));
    }

    let doc = match get_last_pipeline_run(call_context, &project_id, &pipeline_id).await {
        Err(err) => {
            error!("Cannot get badge: {}", err);
            ghlike::render(job_id, ghlike::Status::Unknown)
        }
        Ok(run) => {
            if let Some(job) = run.job(&job_id).await {
                let status = match job.status {
                    worker_lib::executor::JobStatus::Pending => ghlike::Status::Running,
                    worker_lib::executor::JobStatus::Running { .. } => ghlike::Status::Running,
                    worker_lib::executor::JobStatus::Finished { error } => match error {
                        Some(_) => ghlike::Status::Failing,
                        None => ghlike::Status::Passing,
                    },
                };
                ghlike::render(job_id, status)
            } else {
                error!(
                    "No such job {} in pipeline {} in project {}",
                    job_id, pipeline_id, project_id
                );
                ghlike::render(job_id, ghlike::Status::Unknown)
            }
        }
    };

    render_svg(doc).await
}

async fn render_svg(doc: svg::Document) -> Result<impl warp::Reply, warp::Rejection> {
    let mut body: Vec<u8> = Vec::new();

    match svg::write(&mut body, &doc) {
        Err(err) => {
            return Err(warp::reject::custom(InternalServerError::Error(format!(
                "Failed to render svg: {}",
                err
            ))));
        }
        Ok(_) => {}
    }

    let response = warp::http::Response::builder()
        .header("Content-Type", "image/svg+xml")
        .body(body);
    Ok(response)
}

async fn get_last_pipeline_run(
    call_context: call_context::CallContext,
    project_id: impl AsRef<str>,
    pipeline_id: impl AsRef<str>,
) -> Result<Arc<worker_lib::executor::PipelineRun>, anyhow::Error> {
    let executor: &worker_lib::executor::Executor = call_context.state.get()?;

    let runs = executor.runs.lock().await;
    let project_runs = runs
        .get_project_runs(&project_id)
        .ok_or_else(|| anyhow!("No such project: {}", project_id.as_ref()))?;
    let pipeline_runs = project_runs
        .get_pipeline_runs(&pipeline_id)
        .ok_or_else(|| anyhow!("No such pipeline: {}", pipeline_id.as_ref()))?;

    pipeline_runs.last_run().ok_or_else(|| {
        anyhow!(
            "No runs of pipeline {} in project {}",
            pipeline_id.as_ref(),
            project_id.as_ref()
        )
    })
}

mod ghlike {
    use svg::node;
    use svg::node::element::path::Data;
    use svg::node::element::LinearGradient;
    use svg::node::element::Path;
    use svg::node::element::Stop;
    use svg::node::element::TSpan;
    use svg::node::element::Text;
    use svg::Document;

    pub enum Status {
        Failing,
        Running,
        Passing,
        Unknown,
    }

    const FONT: &str = "'DejaVu Sans Mono',Verdana Mono,Geneva Mono,sans-serif-mono";

    pub fn render(job_id: impl AsRef<str>, status: Status) -> svg::Document {
        let r = 4.;
        let h = 20.;

        let tho = 13.;

        let text_margin = 4.;

        let ts = 6.9;

        let icon_vars = ("μCI", 3.);

        let status_vars = match status {
            Status::Failing => ("#D73A49", "#CB2431", "failing"),
            Status::Running => ("#3A59D7", "#2432CD", "running"),
            Status::Passing => ("#34D058", "#28A745", "passing"),
            Status::Unknown => ("#828282", "#616161", "unknown"),
        };
        let base_vars = (job_id.as_ref(),);

        let iw = (icon_vars.0.len() as f32) * ts + icon_vars.1;
        let sw = (status_vars.2.len() as f32) * ts + text_margin * 2.;
        let bw = iw + (base_vars.0.len() as f32) * ts + text_margin * 2.;

        // Base
        let grad_color1 = Stop::new().set("stop-color", "#444D56").set("offset", "0%");

        let grad_color2 = Stop::new()
            .set("stop-color", "#24292E")
            .set("offset", "100%");

        let gradient_base = LinearGradient::new()
            .set("id", "fill-base")
            .set("x1", "50%")
            .set("y1", "0%")
            .set("x2", "50%")
            .set("y2", "100%")
            .add(grad_color1)
            .add(grad_color2);

        let text_base_span = TSpan::new()
            .set("x", text_margin + iw)
            .set("y", tho)
            .add(node::Text::new(base_vars.0));
        let text_base = Text::new().set("fill", "#FFFFFF").add(text_base_span);

        let text_base_shadow_span = TSpan::new()
            .set("x", text_margin + iw)
            .set("y", tho + 1.)
            .add(node::Text::new(base_vars.0));
        let text_base_shadow = Text::new()
            .set("fill", "#010101")
            .set("fill-opacity", ".3")
            .add(text_base_shadow_span);

        let text_icon_span = TSpan::new()
            .set("x", text_margin)
            .set("y", tho)
            .add(node::Text::new(icon_vars.0));
        let text_icon = Text::new()
            .set("fill", "#FFFFFF")
            .set("font-weight", "bold")
            .add(text_icon_span);

        let text_icon_shadow_span = TSpan::new()
            .set("x", text_margin)
            .set("y", tho + 1.)
            .add(node::Text::new(icon_vars.0));
        let text_icon_shadow = Text::new()
            .set("fill", "#010101")
            .set("font-weight", "bold")
            .set("fill-opacity", ".3")
            .add(text_icon_shadow_span);

        let data = Data::new()
            .move_by((r, 0))
            .horizontal_line_by(bw - r)
            .vertical_line_by(h)
            .horizontal_line_by(-(bw - r))
            .elliptical_arc_by((r, r, 0, 0, 1, -r, -r))
            .vertical_line_by(-(h - 2. * r))
            .elliptical_arc_by((r, r, 0, 0, 1, r, -r))
            .close();

        let path_bg = Path::new().set("fill", "url(#fill-base)").set("d", data);

        // Status
        let grad_color1 = Stop::new()
            .set("stop-color", status_vars.0)
            .set("offset", "0%");

        let grad_color2 = Stop::new()
            .set("stop-color", status_vars.1)
            .set("offset", "100%");

        let gradient_status = LinearGradient::new()
            .set("id", "fill-status")
            .set("x1", "50%")
            .set("y1", "0%")
            .set("x2", "50%")
            .set("y2", "100%")
            .add(grad_color1)
            .add(grad_color2);

        let text_status_span = TSpan::new()
            .set("x", bw + text_margin)
            .set("y", tho)
            .add(node::Text::new(status_vars.2));
        let text_status = Text::new().set("fill", "#FFFFFF").add(text_status_span);

        let text_status_shadow_span = TSpan::new()
            .set("x", bw + text_margin)
            .set("y", tho + 1.)
            .add(node::Text::new(status_vars.2));
        let text_status_shadow = Text::new()
            .set("fill", "#010101")
            .set("fill-opacity", ".3")
            .add(text_status_shadow_span);

        let data = Data::new()
            .move_by((bw, 0))
            .horizontal_line_by(sw - r)
            .elliptical_arc_by((r, r, 0, 0, 1, r, r))
            .vertical_line_by(h - 2. * r)
            .elliptical_arc_by((r, r, 0, 0, 1, -r, r))
            .horizontal_line_by(-(sw - r))
            .vertical_line_by(-h)
            .close();

        let path_bg_status = Path::new().set("fill", "url(#fill-status)").set("d", data);

        Document::new()
            .set("width", bw + sw)
            .set("height", h)
            .set("font-family", FONT)
            .set("font-size", 11)
            .add(path_bg)
            .add(gradient_base)
            .add(text_base_shadow)
            .add(text_base)
            .add(text_icon_shadow)
            .add(text_icon)
            .add(gradient_status)
            .add(path_bg_status)
            .add(text_status_shadow)
            .add(text_status)
    }
}
