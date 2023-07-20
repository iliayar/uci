use leptos::*;
use leptos_router::*;

use runner_client::*;

async fn list_projects(config: &crate::config::Config) -> models::ProjectsListResponse {
    // FIXME: Handle error
    api::projects_list(config).await.unwrap()
}

#[component]
pub fn Projects(cx: Scope) -> impl IntoView {
    let config: Signal<crate::config::Config> = expect_context(cx);
    let projects = create_resource(
        cx,
        || (),
        move |_| {
            let config = config();
            async move { list_projects(&config).await }
        },
    );

    move || match projects.read(cx) {
        None => view! {cx, <span class="block">"Loading projects..."</span>}.into_any(),
        Some(resp) => {
            let projects = resp
                .projects
                .into_iter()
                .map(|project| {
                    view! {cx,
		      <li>
		        <A
			  href=format!("projects/{}", project.id.clone())
                          class="hover:bg-button-focus-light hover:dark:bg-button-focus-dark block"
		        >
		          {project.id}
		        </A>
		      </li>
                    }
                })
                .collect::<Vec<_>>();
            if projects.is_empty() {
                view!{cx, <label class="text-op-error-light dark:text-op-error-dark">"No projects"</label> }.into_any()
            } else {
                view! {cx,
                  <ul>
                  {projects.into_view(cx)}
                  </ul>
                }
                .into_any()
            }
        }
    }
}
