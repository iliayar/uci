use leptos::*;

use runner_client::api;

#[derive(Clone)]
enum Status {
    None,
    InProgress,
    Success,
    Error(String),
}

// TODO: Show this somewhere when project not selected
#[component]
pub fn ReloadConfig(cx: Scope) -> impl IntoView {
    let config: Signal<crate::config::Config> = expect_context(cx);

    let (reload_status, set_reload_status) = create_signal(cx, Status::None);

    let reload_config_action = create_action(cx, move |_| {
        let config = config();
        async move {
            set_reload_status(Status::InProgress);

            match api::reload_config(&config).await {
                Ok(_) => set_reload_status(Status::Success),
                Err(e) => set_reload_status(Status::Error(format!("{}", e))),
            }
        }
    });

    let status = move || {
        match reload_status() {
	    Status::None => view!{cx, ""}.into_view(cx),
	    Status::InProgress => view!{cx, "Reloading..."}.into_view(cx),
	    Status::Success =>
		view!{cx, <label class="text-op-success-light dark:text-op-success-dark">"Done"</label>}.into_view(cx),
	    Status::Error(message) =>
		view!{cx, <label class="text-op-error-light dark:text-op-error-dark">"Failed: " {message}</label>}.into_view(cx),
        }
    };

    view! {cx,
     <button
       class="border-border-light dark:border-border-dark hover:bg-button-focus-light hover:dark:bg-button-focus-dark"
       on:click=move |_| reload_config_action.dispatch(())
     >
       "Reload config"
     </button>
     {status}
    }
}
