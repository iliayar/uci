mod app;
mod projects;
mod config;

use leptos::*;

use app::*;

fn main() {
    let config = config::Config {
        runner_url: "http://localhost:8000/api".to_string(),
    };

    mount_to_body(|cx| {
	provide_context(cx, config);
	view! { cx,  <App/> }
    })
}
