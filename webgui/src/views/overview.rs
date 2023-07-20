use leptos::*;

#[component]
pub fn Overview(cx: Scope) -> impl IntoView {
    let project: Signal<Option<crate::types::ProjectId>> = expect_context(cx);

    view! {cx,
      <div>
        <label>"Project: "</label><code class="font-bold">{move || project().unwrap().0}</code>
      </div>
      <hr/>
    }
}
