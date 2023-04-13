use futures::{FutureExt, StreamExt};
use std::{collections::HashMap, convert::Infallible, sync::Arc};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::{Filter, Rejection};

use super::{
    config,
    context::{self, Context},
    handlers,
};
use warp::hyper::StatusCode;

use log::*;

pub type ContextPtr<PM> = Arc<Context<PM>>;

pub fn runner<PM: config::ProjectsManager>(
    context: Context<PM>,
) -> impl Filter<Extract = impl warp::Reply, Error = Infallible> + Clone {
    let context = Arc::new(context);
    ping()
        .or(handlers::call::filter(context.clone()))
        .or(handlers::reload_config::filter(context.clone()))
        .or(handlers::update_repo::filter(context.clone()))
        .or(handlers::list_projects::filter(context.clone()))
        .or(ws(context.clone()))
        .recover(report_rejection)
}

pub fn ping() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("ping").and(warp::get()).map(|| StatusCode::OK)
}

pub fn ws<PM: config::ProjectsManager>(
    context: ContextPtr<PM>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("ws" / String)
        .and(warp::ws())
        .and(with_context(context))
        .and_then(ws_client)
}

async fn ws_client<PM: config::ProjectsManager>(
    client_id: String,
    ws: warp::ws::Ws,
    context: ContextPtr<PM>,
) -> Result<impl warp::Reply, warp::Rejection> {
    debug!("Handling ws client {}", client_id);
    let client = context.ws_clients.lock().await.remove(&client_id);
    match client {
        Some(client) => Ok(ws.on_upgrade(move |socket| ws_client_connection(socket, client))),
        None => Err(warp::reject::not_found()),
    }
}

async fn ws_client_connection(socket: warp::ws::WebSocket, client: context::WsClient) {
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

pub fn with_call_context<PM: config::ProjectsManager>(
    context: ContextPtr<PM>,
) -> impl Filter<Extract = (handlers::CallContext<PM>,), Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_validation())
        .and(with_context(context))
        .map(handlers::CallContext::for_handler)
}

pub fn with_context<PM: config::ProjectsManager>(
    context: ContextPtr<PM>,
) -> impl Filter<Extract = (ContextPtr<PM>,), Error = Infallible> + Clone {
    warp::any().map(move || context.clone())
}

#[derive(Debug)]
pub enum Unauthorized {
    UnsupportedAuthorizationMethod(String),
    MethodIsNotSepcified,
    TokenIsNotSpecified,
    TokenIsUnauthorized,
}

impl warp::reject::Reject for Unauthorized {}

// FIXME: Make one error, meaningfull
#[derive(Debug)]
pub enum InternalServerError {
    Error(String),
}

impl warp::reject::Reject for InternalServerError {}

pub fn with_validation() -> impl Filter<Extract = (Option<String>,), Error = Rejection> + Clone {
    warp::header::optional("Authorization").and_then(|auth: Option<String>| async move {
        if let Some(auth) = auth {
            let mut split = auth.split_whitespace();
            if let Some(method) = split.next() {
                if method != "Api-Key" {
                    Err(warp::reject::custom(
                        Unauthorized::UnsupportedAuthorizationMethod(method.to_string()),
                    ))
                } else {
                    if let Some(token) = split.next() {
                        Ok(Some(token.to_string()))
                    } else {
                        Err(warp::reject::custom(Unauthorized::TokenIsNotSpecified))
                    }
                }
            } else {
                Err(warp::reject::custom(Unauthorized::MethodIsNotSepcified))
            }
        } else {
            Ok(None)
        }
    })
}

pub async fn report_rejection(r: Rejection) -> Result<impl warp::Reply, Infallible> {
    if let Some(auth_error) = r.find::<Unauthorized>() {
        let message = match auth_error {
            Unauthorized::UnsupportedAuthorizationMethod(method) => {
                format!("Unsupported auth method {}", method)
            }
            Unauthorized::MethodIsNotSepcified => format!("Auth method is not specified"),
            Unauthorized::TokenIsNotSpecified => format!("Auth token is not specified"),
            Unauthorized::TokenIsUnauthorized => {
                format!("Specified token is unauthrized for this action")
            }
        };
        Ok(warp::reply::with_status(
            warp::reply::json(&common::runner::ErrorResponse { message }),
            StatusCode::UNAUTHORIZED,
        ))
    } else if let Some(internal_server_error) = r.find::<InternalServerError>() {
        let message = match internal_server_error {
            InternalServerError::Error(err) => err.clone(),
        };
        Ok(warp::reply::with_status(
            warp::reply::json(&common::runner::ErrorResponse { message }),
            StatusCode::INTERNAL_SERVER_ERROR,
        ))
    } else {
        if r.is_not_found() {
            Ok(warp::reply::with_status(
                warp::reply::json(&common::runner::ErrorResponse {
                    message: "Not found".to_string(),
                }),
                StatusCode::NOT_FOUND,
            ))
        } else {
            Ok(warp::reply::with_status(
                warp::reply::json(&common::runner::ErrorResponse {
                    message: "Unknown error".to_string(),
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}
