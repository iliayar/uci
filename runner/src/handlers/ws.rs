use common::run_context::WsClientReciever;
use futures::{SinkExt, StreamExt};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::Filter;

use runner_lib::{call_context, config};

use crate::filters::with_call_context;

use log::*;

pub fn filter<PM: config::ProjectsManager>(
    deps: call_context::Deps<PM>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("ws" / String)
        .and(warp::ws())
        .and(with_call_context(deps))
        .and_then(ws_client)
}

async fn ws_client<PM: config::ProjectsManager>(
    run_id: String,
    ws: warp::ws::Ws,
    context: call_context::CallContext<PM>,
) -> Result<impl warp::Reply, warp::Rejection> {
    debug!("Handling ws client {}", run_id);
    let client = context.make_out_channel(run_id).await;
    match client {
        Some(client) => Ok(ws.on_upgrade(move |socket| ws_client_connection(socket, client))),
        None => Err(warp::reject::not_found()),
    }
}

async fn ws_client_connection(socket: warp::ws::WebSocket, rx: WsClientReciever) {
    // NOTE: Do not care of receiving messages
    let (mut client_ws_sender, _) = socket.split();
    let mut client_rcv = UnboundedReceiverStream::new(rx);
    debug!("Running ws sending");
    tokio::task::spawn(async move {
        while let Some(msg) = client_rcv.next().await {
            match msg {
                Ok(msg) => {
                    if let Err(e) = client_ws_sender
                        .send(warp::ws::Message::text(msg.to_string()))
                        .await
                    {
                        error!("Error sending websocket msg: {}", e);
                        break;
                    }
                }
                Err(err) => {
                    error!("Failed to receive ws msg: {}", err);
                    break;
                }
            }
        }
        debug!("Closing ws connection");
        client_rcv.close();
        if let Err(err) = client_ws_sender.close().await {
            error!("Failed to close ws sender: {}", err);
        }
    });
}
