use leptos::*;

use runner_client::api;

#[derive(Clone)]
enum CallStatus {
    None,
    InProgress,
    Done(String),
    Error(String),
}

#[component]
pub fn Action(cx: Scope, name: String) -> impl IntoView {
    let project: Signal<Option<crate::types::ProjectId>> = expect_context(cx);
    let config: Signal<crate::config::Config> = expect_context(cx);

    let (call_status, set_call_status) = create_signal(cx, CallStatus::None);

    let trigger_id = name.clone();
    let call_action = create_action(cx, move |_| {
        let trigger_id = trigger_id.clone();
        let project = project().unwrap().0;
        let config = config();
        async move {
            let request = models::CallRequest {
                project_id: project,
                dry_run: None,
                trigger_id,
            };

            set_call_status(CallStatus::InProgress);
            let response = api::action_call(&config, &request).await;

            match response {
                Ok(resp) => {
                    set_call_status(CallStatus::Done(resp.run_id));
                }
                Err(err) => {
                    set_call_status(CallStatus::Error(format!("{}", err)));
                }
            }
        }
    });

    let call_status_view = move || {
        match call_status.get() {
            CallStatus::None => view! {cx, }.into_view(cx),
	    CallStatus::InProgress => view!{cx, <span>"Calling..."</span> }.into_view(cx),
	    CallStatus::Done(run_id) =>
	        view!{cx,
	          <span class="text-op-success-light dark:text-op-success-dark">"Called: " <code class="font-bold">{run_id}</code></span>
	        }.into_view(cx),
	    CallStatus::Error(error) =>
	        view!{cx,
	          <span class="text-op-error-light dark:text-op-error-dark">"Error: " {error}</span>
	        }.into_view(cx),
        }
    };

    view! {cx,
      <div class="w-full p-1">
        <div class="flex flex-row justify-between items-center">
          <div>
            <button
              class="hover:bg-button-focus-light hover:dark:bg-button-focus-dark mr-1"
              on:click=move |_| call_action.dispatch(())
            >
               <i class="fa-solid fa-play text-run-button-light dark:text-run-button-dark p-1"></i>
            </button>
            <label>"Action: "</label><code class="font-bold">{name}</code>
          </div>
          <div>
           {call_status_view}
          </div>
        </div>
      </div>
    }
}

#[component]
pub fn Actions(cx: Scope) -> impl IntoView {
    let project: Signal<Option<crate::types::ProjectId>> = expect_context(cx);
    let config: Signal<crate::config::Config> = expect_context(cx);

    let actions = create_resource(
        cx,
        move || (project(), config()),
        |(project, config)| async move {
            let project = project.unwrap().0;
            api::list_actions(&config, project)
                .await
                .map_err(|err| format!("{}", err))
        },
    );

    let actions = move || {
        match actions.read(cx) {
        None => view! {cx, <span>"Loading actions..."</span> }.into_view(cx),
        Some(Ok(resp)) => resp
            .actions
            .into_iter()
            .map(|action| {
                view! {cx,
                  <Action name={action.id} />
		  <hr/>
                }
            })
            .collect::<Vec<_>>()
            .into_view(cx),
        Some(Err(err)) => {
            view! {cx,
              <span class="text-op-error-light dark:text-op-error-dark">"Failed to load services list: " {err}</span>
            }.into_view(cx)
        }
    }
    };

    view! {cx,
     <div class="flex flex-col space-y-2">
       {actions}
     </div>
    }
}
