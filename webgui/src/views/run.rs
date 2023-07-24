use std::{collections::hash_map::DefaultHasher, hash::Hasher};

use leptos::*;
use leptos_router::*;
use runner_client::RunnerClientConfig;

use futures_util::stream::StreamExt;
use ws_stream_wasm::{WsMessage, WsStream};

use chrono::*;

fn rand_color<I: std::hash::Hash>(item: &I) -> (u8, u8, u8) {
    let mut hash = DefaultHasher::new();
    item.hash(&mut hash);
    let res: u32 = hash.finish() as u32;

    let r: u8 = ((res & (0xff << 0)) >> 0) as u8;
    let g: u8 = ((res & (0xff << 8)) >> 8) as u8;
    let b: u8 = ((res & (0xff << 16)) >> 16) as u8;

    (r, g, b)
}

fn format_color((r, g, b): (u8, u8, u8)) -> String {
    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

#[component]
pub fn LogLine(
    cx: Scope,
    params: (models::LogType, String, DateTime<Utc>, String, String),
) -> impl IntoView {
    let (t, text, ts, _pipeline, job) = params;

    let text_raw_html = ansi_to_html::convert_escaped(&text).unwrap();
    let text = view!{cx, <div class="inline"></div>};
    text.set_inner_html(&text_raw_html);

    let ts = ts.format("%Y-%m-%d %H:%M:%S").to_string();
    let ts = view! {cx, <code class="text-black-1-light dark:text-black-1-dark pr-1">{ts}</code>};

    let job_color_style = format!("color: {}", format_color(rand_color(&job)));
    let job =
        view! {cx, <div class="inline pr-1"><code style=job_color_style>"[" {job} "]"</code></div>};

    let text = match t {
        models::LogType::Regular => view! {cx, <code>{text}</code>},
        models::LogType::Error => {
            view! {cx, <code class="text-red-0-light dark:text-red-0-dark">{text}</code>}
        }
        models::LogType::Warning => {
            view! {cx, <code class="text-yellow-0-light dark:text-yellow-0-dark">{text}</code>}
        }
    };

    view! {cx, <div>{ts} {job} {text}</div>}
}

#[component]
pub fn Run(cx: Scope) -> impl IntoView {
    let params = use_params_map(cx);

    let project: Signal<Option<crate::types::ProjectId>> = expect_context(cx);
    let run = Signal::derive(cx, move || {
        params.with(|params| params.get("run").cloned().unwrap_or("(none)".to_string()))
    });
    let pipeline = Signal::derive(cx, move || {
        params.with(|params| {
            params
                .get("pipeline")
                .cloned()
                .unwrap_or("(none)".to_string())
        })
    });

    let set_show_runs: WriteSignal<super::runs::ShowRuns> = expect_context(cx);
    set_show_runs(super::runs::ShowRuns(false));

    let config: Signal<crate::config::Config> = expect_context(cx);

    let (logs, set_logs) = create_signal(
        cx,
        Vec::<(
            usize,
            (models::LogType, String, DateTime<Utc>, String, String),
        )>::new(),
    );

    let add_log =
        move |t: models::LogType, log: String, ts: DateTime<Utc>, pipeline: String, job: String| {
            set_logs.update(move |logs| {
                logs.push((logs.len(), (t, log, ts, pipeline, job)));
            });
        };

    let watch_logs = create_action(cx, move |_: &()| {
        let config = config();
        let project = project().unwrap();
        let pipeline = pipeline();
        let run = run();

        async move {
            let query = models::RunsLogsRequestQuery {
                run: run.clone(),
                project: project.0,
                pipeline: pipeline.clone(),
            };

            let logs_run_id = runner_client::api::run_logs(&config, &query).await.unwrap();

            let events_follow = ws(&config, run).await;
            let events = ws(&config, logs_run_id.run_id).await;

            let handle_events = |mut events: WsStream, since: Option<DateTime<Utc>>| async move {
                let mut last_ts: Option<DateTime<Utc>> = None;
                while let Some(event) = events.next().await {
                    match event {
                        WsMessage::Text(message) => {
                            let message = serde_json::from_str::<models::PipelineMessage>(&message);
                            match message {
                                Ok(models::PipelineMessage::Log {
                                    t,
                                    text,
                                    timestamp,
                                    pipeline,
                                    job_id,
                                }) => {
                                    if let Some(ts) = last_ts {
                                        last_ts = Some(ts.max(timestamp));
                                    } else {
                                        last_ts = Some(timestamp);
                                    }

                                    if let Some(since) = since.as_ref() {
                                        if &timestamp <= since {
                                            continue;
                                        }
                                    }
                                    add_log(t, text, timestamp, pipeline, job_id);
                                }
                                _ => {}
                            }
                        }
                        _ => {
                            log!("Unhadled message: {:?}", event)
                        }
                    };
                }

                return last_ts;
            };

            let mut last_ts = None;
            if let Some(events) = events {
                last_ts = handle_events(events, None).await;
            }

            if let Some(events_follow) = events_follow {
                handle_events(events_follow, last_ts).await;
            }
        }
    });

    create_effect(cx, move |_| watch_logs.dispatch(()));

    view! {cx,
      <div class="flex flex-col">
        <div>
          <label>"Run: "</label><code class="font-bold">{run}</code>
        </div>
        <div>
          <label>"Pipeline: "</label><code class="font-bold">{pipeline}</code>
        </div>
        <hr/>
    <div class="flex flex-col m-2 p-1 grow h-full bg-white-1-light dark:bg-white-1-dark">
       <For
         each=logs
         key=|log| log.0
         view=|cx, log| view!{cx, <LogLine params=log.1 />}
       />
    </div>
      </div>
    }
}

async fn ws(config: &crate::config::Config, run_id: impl AsRef<str>) -> Option<WsStream> {
    let url = format!("{}/ws/{}", config.ws_runner_url().unwrap(), run_id.as_ref());
    let (_, wsio) = ws_stream_wasm::WsMeta::connect(url, None).await.ok()?;
    Some(wsio)
}
