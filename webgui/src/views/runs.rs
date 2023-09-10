use leptos::*;
use leptos_router::*;

use runner_client::api;

#[component]
pub fn RunItem(cx: Scope, run: models::Run) -> impl IntoView {
    let since = run.started.format("%Y-%m-%d %H:%M:%S").to_string();

    let status = match run.status {
        models::RunStatus::Running => {
            view! {cx, <label class="text-op-in-progress-light dark:text-op-in-progress-dark">"Running"</label>}
        }
        models::RunStatus::Finished(finished_status) => match finished_status {
            models::RunFinishedStatus::Success => {
                view! {cx, <label class="text-op-success-light dark:text-op-success-dark">"Finished"</label>}
            }
            models::RunFinishedStatus::Error { message } => {
                view! {cx, <label class="text-op-error-light dark:text-op-error-dark">"Failed: " {message}</label>}
            }
            models::RunFinishedStatus::Canceled => {
                view! {cx, <label class="text-op-warning-light dark:text-op-warning-dark">"Canceled"</label>}
            }
            models::RunFinishedStatus::Displaced => {
                view! {cx, <label class="text-fg2-light dark:text-fg2-dark">"Displaced"</label>}
            }
        },
    };

    view! {cx,
      <div class="w-full p-1">
        <div class="flex flex-row justify-between items-center">
          <div class="flex flex-col">
            <div><label>"Run: "</label><code class="font-bold">{run.run_id}</code></div>
            <div><label>"Pipeline: "</label><code class="font-bold">{run.pipeline}</code></div>
          </div>
          <div>
            {status}
            <label class="ml-1 italic text-fg2-light dark:text-fd2-dark">"Started " {since}</label>
          </div>
        </div>
      </div>
    }
}

#[derive(Clone)]
pub struct ShowRuns(pub bool);

#[component]
pub fn Runs(cx: Scope) -> impl IntoView {
    let project: Signal<Option<crate::types::ProjectId>> = expect_context(cx);
    let config: Signal<crate::config::Config> = expect_context(cx);

    let runs = create_resource(
        cx,
        move || (project(), config()),
        |(project, config)| async move {
            let pipeline = None;
            api::list_runs(&config, Some(project.unwrap().0), pipeline)
                .await
                .map_err(|err| format!("{}", err))
        },
    );

    let runs = move || {
        match runs.read(cx) {
            None => view! {cx, <span>"Loading runs..."</span> }.into_view(cx),
            Some(Ok(resp)) => resp
                .runs
                .into_iter()
                .map(|run| {
		    let href = format!("{}/{}", run.run_id, run.pipeline);
                    view! {cx,
		      <A href=href class="hover:bg-button-focus-light hover:dark:bg-button-focus-dark">
	        	<RunItem run={run} />
	    	      </A>
	    	      <hr/>
                    }
                })
                .collect::<Vec<_>>()
                .into_view(cx),
            Some(Err(err)) => {
                view! {cx,
                  <span class="text-op-error-light dark:text-op-error-dark">"Failed to load runs list: " {err}</span>
                }.into_view(cx)
            }
        }
    };

    let (show_runs, set_show_runs) = create_signal(cx, ShowRuns(true));
    provide_context(cx, set_show_runs);

    view! {cx,
     <div class="flex flex-col space-y-2 h-full">
       {move || if show_runs().0 { runs() } else { view!{cx, }.into_view(cx) }}
       <Outlet />
     </div>
    }
}

#[component]
pub fn NoRunSelected(cx: Scope) -> impl IntoView {
    let set_show_runs: WriteSignal<super::runs::ShowRuns> = expect_context(cx);
    set_show_runs(super::runs::ShowRuns(true));

    view!{cx, }
}
