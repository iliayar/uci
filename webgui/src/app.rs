use super::projects::Projects;
use leptos::*;

#[component]
pub fn App(cx: Scope) -> impl IntoView {
    view! { cx,
        <Projects />
    }
}
