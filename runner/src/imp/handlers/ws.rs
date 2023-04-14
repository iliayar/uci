use futures::{FutureExt, StreamExt};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::Filter;

use crate::imp::{
    config,
    filters::{with_call_context, Deps},
};

use log::*;

pub fn filter<PM: config::ProjectsManager>(
    deps: Deps<PM>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("ws" / String)
        .and(warp::ws())
        .and(with_call_context(deps))
        .and_then(ws_client)
}

async fn ws_client<PM: config::ProjectsManager>(
    client_id: String,
    ws: warp::ws::Ws,
    context: super::CallContext<PM>,
) -> Result<impl warp::Reply, warp::Rejection> {
    debug!("Handling ws client {}", client_id);
    let client = context.ws_clients.lock().await.remove(&client_id);
    match client {
        Some(client) => Ok(ws.on_upgrade(move |socket| ws_client_connection(socket, client))),
        None => Err(warp::reject::not_found()),
    }
}

async fn ws_client_connection(socket: warp::ws::WebSocket, client: super::WsClient) {
    // NOTE: Do not care of receiving messages
    let (client_ws_sender, _) = socket.split();
    let client_rcv = UnboundedReceiverStream::new(client.rx);
    debug!("Running ws sending");
    tokio::task::spawn(client_rcv.forward(client_ws_sender).map(|result| {
        if let Err(e) = result {
            error!("Error sending websocket msg: {}", e);
        }
    }));
}
