mod lib;
use log::*;

#[tokio::main]
async fn main() {
    match lib::App::init().await {
        Ok(app) => app.run().await,
        Err(err) => {
	    error!("App exited with error: {}", err)
	},
    }
}
