#![allow(opaque_hidden_inferred_bound)]
#![allow(unused_variables)]

mod app;
mod filters;
mod handlers;

use log::*;

#[tokio::main]
async fn main() {
    match app::App::init().await {
        Ok(app) => app.run().await,
        Err(err) => {
            error!("App exited with error: {}", err)
        }
    }
}
