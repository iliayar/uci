use futures::{FutureExt, StreamExt};
use std::{collections::HashMap, convert::Infallible, sync::Arc};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::{Filter, Rejection};

use super::{
    context::{Context, ContextError},
    handlers,
};
use warp::hyper::StatusCode;

use log::*;

#[derive(Clone)]
pub struct ContextStore {
    context: Arc<Context>,
    ws_clients: Arc<Mutex<HashMap<String, WsClient>>>,
}

pub struct WsClient {
    rx: mpsc::UnboundedReceiver<Result<warp::ws::Message, warp::Error>>,
}

pub struct WsResult {
    pub client_id: String,
    pub ws_output: WsOutput,
}

pub struct WsOutput {
    tx: mpsc::UnboundedSender<Result<warp::ws::Message, warp::Error>>,
}

impl WsOutput {
    pub async fn send<T: serde::Serialize>(&self, msg: T) {
        let content = match serde_json::to_string(&msg) {
            Err(err) => {
                error!("Failed to encode msg for ws: {}", err);
                return;
            }
            Ok(content) => content,
        };
        if let Err(err) = self.tx.send(Ok(warp::ws::Message::text(content))) {
            error!("Failed to send ws message {}", err);
        }
    }
}

impl ContextStore {
    pub async fn create_ws(&self) -> WsResult {
        let client_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::unbounded_channel();
        let client = WsClient { rx };
        self.ws_clients
            .lock()
            .await
            .insert(client_id.clone(), client);
        debug!("New ws client registerd: {}", client_id);
        WsResult {
            client_id,
            ws_output: WsOutput { tx },
        }
    }
}

impl ContextStore {
    pub fn new(context: Context) -> ContextStore {
        ContextStore {
            context: Arc::new(context),
            ws_clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn context(&self) -> &Context {
        self.context.as_ref()
    }
}

pub fn runner(
    context_store: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> impl Filter<Extract = impl warp::Reply, Error = Infallible> + Clone {
    ping()
        .or(handlers::call::filter(
            context_store.clone(),
            worker_context.clone(),
        ))
        .or(handlers::reload_project::filter(
            context_store.clone(),
            worker_context.clone(),
        ))
        .or(handlers::reload_config::filter(
            context_store.clone(),
            worker_context.clone(),
        ))
        .or(handlers::update_repo::filter(
            context_store.clone(),
            worker_context.clone(),
        ))
        .or(handlers::list_projects::filter(
            context_store.clone(),
            worker_context.clone(),
        ))
        .or(ws(context_store.clone()))
        .recover(report_rejection)
}

pub fn ping() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("ping").and(warp::get()).map(|| StatusCode::OK)
}

pub fn ws(
    context: ContextStore,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("ws" / String)
        .and(warp::ws())
        .and(with_context(context))
        .and_then(ws_client)
}

async fn ws_client(
    client_id: String,
    ws: warp::ws::Ws,
    store: ContextStore,
) -> Result<impl warp::Reply, warp::Rejection> {
    debug!("Handling ws client {}", client_id);
    let client = store.ws_clients.lock().await.remove(&client_id);
    match client {
        Some(client) => Ok(ws.on_upgrade(move |socket| ws_client_connection(socket, client))),
        None => Err(warp::reject::not_found()),
    }
}

async fn ws_client_connection(socket: warp::ws::WebSocket, client: WsClient) {
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

pub fn with_call_context(
    context: ContextStore,
    worker_context: Option<worker_lib::context::Context>,
) -> impl Filter<Extract = (CallContext,), Error = warp::Rejection> + Clone {
    warp::any()
        .and(with_validation())
        .and(with_context(context))
        .and(with_worker_context(worker_context))
        .map(CallContext::for_handler)
}

pub fn with_context(
    context: ContextStore,
) -> impl Filter<Extract = (ContextStore,), Error = Infallible> + Clone {
    warp::any().map(move || context.clone())
}

pub fn with_worker_context(
    worker_context: Option<worker_lib::context::Context>,
) -> impl Filter<Extract = (Option<worker_lib::context::Context>,), Error = Infallible> + Clone {
    warp::any().map(move || worker_context.clone())
}

#[derive(Debug)]
pub enum Unauthorized {
    UnsupportedAuthorizationMethod(String),
    MethodIsNotSepcified,
    TokenIsNotSpecified,
    TokenIsUnauthorized,
}

impl warp::reject::Reject for Unauthorized {}

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

pub struct CallContext {
    pub token: Option<String>,
    pub check_permisions: bool,
    pub worker_context: Option<worker_lib::context::Context>,
    pub store: ContextStore,
    pub ws: Option<WsOutput>,
}

impl CallContext {
    pub async fn check_authorized<S: AsRef<str>>(
        &self,
        project_id: Option<S>,
        action: super::config::ActionType,
    ) -> Result<(), warp::Rejection> {
        if !self.check_allowed(project_id, action).await {
            Err(warp::reject::custom(Unauthorized::TokenIsUnauthorized))
        } else {
            Ok(())
        }
    }

    fn for_handler(
        token: Option<String>,
        store: ContextStore,
        worker_context: Option<worker_lib::context::Context>,
    ) -> CallContext {
        CallContext {
            token,
            check_permisions: true,
            worker_context,
            store,
            ws: None,
        }
    }

    pub async fn check_allowed<S: AsRef<str>>(
        &self,
        project_id: Option<S>,
        action: super::config::ActionType,
    ) -> bool {
        if !self.check_permisions {
            return true;
        }
        self.store
            .context()
            .config()
            .await
            .service_config
            .check_allowed(self.token.as_ref(), project_id, action)
    }

    pub async fn get_actions(
        &self,
        event: super::config::ActionEvent,
    ) -> Result<super::config::MatchedActions, super::config::ExecutionError> {
        self.store
            .context()
            .config()
            .await
            .get_projects_actions(event)
            .await
    }

    pub async fn to_execution_context(self) -> super::config::ExecutionContext {
        super::config::ExecutionContext {
            token: self.token,
            check_permissions: self.check_permisions,
            worker_context: self.worker_context,
            config: self.store.context().config().await,
        }
    }

    pub async fn send<T: serde::Serialize>(&self, msg: T) {
        if let Some(ws_output) = self.ws.as_ref() {
            ws_output.send(msg).await;
        }
    }
}
