use leptos::*;

use runner_client::*;

async fn list_projects(config: &crate::config::Config) -> models::ProjectsListResponse {
    // FIXME: Handle error
    api::projects_list(config).await.unwrap()
}

#[component]
pub fn Projects(cx: Scope) -> impl IntoView {
    let config = use_context::<crate::config::Config>(cx).expect("Context should exist");
    let projects = create_resource(
        cx,
        || (),
        move |_| {
            let config = config.clone();
            async move { list_projects(&config).await }
        },
    );

    view! {cx,
       "Projects:" <br/>
       {move || match projects.read(cx) {
           None => view! { cx, "Loading..." }.into_view(cx),
           Some(resp) => resp.projects.into_iter()
        .map(|p| view!{ cx, {p.id} <br/>})
       .collect::<Vec<_>>().into_view(cx),
       }
        }
    }
}
