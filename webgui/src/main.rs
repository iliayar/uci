mod app;
mod projects;
mod config;
mod views;
mod routes;
mod body;
mod auth;
mod types;

use leptos::*;

use app::*;

fn main() {
    let runner_url = env!("UCI_BASE_URL");
    let ws_runner_url = env!("UCI_WS_BASE_URL");

    mount_to_body(|cx| {
	view! { cx,  <App runner_url={runner_url} ws_runner_url={ws_runner_url}/> }
    })
}
