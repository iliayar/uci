use leptos::*;

use crate::routes::UiRoutes;

#[component]
pub fn App(cx: Scope, runner_url: String, ws_runner_url: String) -> impl IntoView {
    let runner_url = super::body::RunnerUrl {
	url: runner_url,
	ws_url: ws_runner_url,
    };
    provide_context(cx, runner_url);

    view!{cx, <UiRoutes />}
}
