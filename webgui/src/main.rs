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
    let runner_url = "http://localhost:3000/api".to_string();
    let ws_runner_url = "ws://localhost:3000/api".to_string();

    mount_to_body(|cx| {
	view! { cx,  <App runner_url={runner_url} ws_runner_url={ws_runner_url}/> }
    })
}
