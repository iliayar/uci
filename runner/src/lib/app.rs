pub struct App {}

use warp::Filter;

use super::filters;

impl App {
    pub async fn init() -> App {
        let app = App {};

        pretty_env_logger::init();

        app
    }

    pub async fn run(self) {
        let api = filters::runner();
        let routes = api.with(warp::log("runner"));

	warp::serve(routes).run(([127, 0, 0, 1], 8080)).await;
    }
}
