use leptos::*;

use runner_client::RunnerClientConfig;

#[component]
pub fn Auth(cx: Scope, set_token: WriteSignal<Option<String>>) -> impl IntoView {
    let config: Signal<crate::config::Config> = expect_context(cx);

    move || {
        if config().token().is_some() {
            let deauth = move |_| {
                set_token(None);
            };
            view! {cx,
              <div class="pr-5">
                <button class="dark:border-border-dark hover:bg-button-focus-light hover:dark:bg-button-focus-dark" on:click=deauth>"Deauth"</button>
              </div>
            }
        } else {
            let token_input: NodeRef<html::Input> = create_node_ref(cx);

            let auth = move || {
                let token: String = token_input().expect("Input should be loaded").value();
                set_token(Some(token));
            };

            let auth_button = move |_| auth();

            let auth_enter = move |event: ev::KeyboardEvent| {
                if event.key() == "Enter" {
                    auth();
                }
            };
            view! {cx,
              <div class="pr-4">
                <input type="text" placeholder="Token" class="border-x-2 px-1 dark:border-border-dark bg-bg-light dark:bg-bg-dark" node_ref=token_input on:keypress=auth_enter />
                <button class="px-2 border-r-2 text-center dark:border-border-dark hover:bg-button-focus-light hover:dark:bg-button-focus-dark" on:click=auth_button>"Auth"</button>
              </div>
            }
        }
    }
}
