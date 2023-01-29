
mod lib;

#[tokio::main]
async fn main() {
    lib::App::init().await.run().await;
}
