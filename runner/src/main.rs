#[allow(unused_variables)]
#[allow(opaque_hidden_inferred_bound)]
#[allow(dead_code)]
mod imp;
use log::*;

#[tokio::main]
async fn main() {
    match imp::App::init().await {
        Ok(app) => app.run().await,
        Err(err) => {
            error!("App exited with error: {}", err)
        }
    }
}
