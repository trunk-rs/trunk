use std::borrow::Cow;
use std::str::FromStr;

use anyhow::Error;
use futures::{SinkExt, StreamExt};
use hyper::Client;
use lazy_static::lazy_static;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;
use warp::filters::BoxedFilter;
use warp::http::{Request, Uri};
use warp::hyper::client::connect::dns::GaiResolver;
use warp::hyper::client::HttpConnector;
use warp::hyper::Body;
use warp::reject::Reject;
use warp::ws::{self, Message as WarpMessage};
use warp::{http, hyper, reject, Filter, Rejection, Reply};

use crate::common::ERROR;

#[derive(Debug)]
pub struct ProxyRejection(pub Error);

impl Reject for ProxyRejection {}

pub fn extract_request() -> impl Filter<Extract = (http::Request<warp::hyper::Body>,), Error = Rejection> + Copy {
    warp::method()
        .and(warp::path::full())
        .and(warp::header::headers_cloned())
        .and(warp::body::bytes())
        .and_then(
            |method: http::Method, path: warp::path::FullPath, headers: http::HeaderMap, body| async move {
                let mut req = http::Request::builder()
                    .method(method)
                    .uri(path.as_str())
                    .body(warp::hyper::Body::from(body))
                    .map_err(|e| reject::custom(ProxyRejection(Error::from(e))))?;
                {
                    *req.headers_mut() = headers;
                }
                Ok::<_, Rejection>(req)
            },
        )
}

lazy_static! {
    // ideally this should use reqwest but I don't know of any way to convert
    // `http::Request<hyper::Body>` into `reqwest::Request`.
    // For any other usage, consider depending upon and using reqwest instead
    static ref CLIENT: Client<HttpConnector<GaiResolver>, Body> = Client::new();
}

async fn http_proxy_handler(mut request: Request<hyper::Body>, proxy_to: String) -> Result<warp::reply::Response, Rejection> {
    let uri = request.uri();
    let proxy_to = proxy_to.strip_suffix("/").unwrap_or(&proxy_to);

    *request.uri_mut() = Uri::from_str(&format!("{}{}", proxy_to, uri)).unwrap();

    CLIENT.request(request).await.map_err(|e| reject::custom(ProxyRejection(Error::from(e))))
}

pub fn http_proxy(path: BoxedFilter<()>, proxy_to: String) -> impl Filter<Extract = (warp::reply::Response,), Error = warp::Rejection> + Clone {
    path.and(extract_request())
        .and(warp::any().map(move || proxy_to.clone()))
        .and_then(http_proxy_handler)
}

async fn ws_proxy_handler(ws: ws::Ws, redirect_to: String) -> Result<warp::reply::Response, warp::Rejection> {
    let resp = ws.on_upgrade(|ws_conn| async move {
        let (mut warp_sink, mut warp_source) = ws_conn.split();
        let (mut remote_sink, mut remote_source) = match tokio_tungstenite::connect_async(redirect_to).await {
            Ok(ws) => ws.0.split(),
            Err(e) => {
                eprintln!("{} error occurred while opening proxy websocket: {}", ERROR, e);
                if warp_sink.send(WarpMessage::close()).await.is_ok() {
                    warp_sink.flush().await.unwrap();
                };
                return;
            }
        };

        let redirect_warp = async move {
            while let Some(Ok(msg)) = remote_source.next().await {
                let msg = match msg {
                    TungsteniteMessage::Binary(data) => WarpMessage::binary(data),
                    TungsteniteMessage::Text(data) => WarpMessage::text(data),
                    TungsteniteMessage::Ping(data) => WarpMessage::ping(data),
                    TungsteniteMessage::Pong(_) => continue,
                    TungsteniteMessage::Close(Some(frame)) => WarpMessage::close_with(frame.code, frame.reason),
                    TungsteniteMessage::Close(None) => WarpMessage::close(),
                };
                if warp_sink.send(msg).await.is_ok() {
                    warp_sink.flush().await.unwrap();
                }
            }
        };
        let redirect_remote = async move {
            while let Some(Ok(msg)) = warp_source.next().await {
                let msg = if msg.is_binary() {
                    TungsteniteMessage::binary(msg.into_bytes())
                } else if msg.is_text() {
                    match msg.to_str() {
                        Ok(text) => TungsteniteMessage::text(text),
                        Err(err) => {
                            eprintln!("error extracting proxied WebSocket text {:?}", err);
                            continue;
                        }
                    }
                } else if msg.is_close() {
                    let frame = msg.close_frame().map(|(code, reason)| CloseFrame {
                        code: code.into(),
                        reason: Cow::from(reason.to_owned()),
                    });
                    TungsteniteMessage::Close(frame)
                } else if msg.is_ping() {
                    TungsteniteMessage::Ping(msg.into_bytes())
                } else if msg.is_pong() {
                    TungsteniteMessage::Pong(msg.into_bytes())
                } else {
                    eprintln!("unrecognized message from proxied WebSocket: {:?}", msg);
                    continue;
                };

                if remote_sink.send(msg).await.is_ok() {
                    remote_sink.flush().await.unwrap();
                }
            }
        };

        let handle1 = tokio::spawn(redirect_warp);
        let handle2 = tokio::spawn(redirect_remote);

        if let Err(e) = tokio::try_join!(handle1, handle2) {
            eprintln!("{} WebSocket proxy error: {}", ERROR, e)
        };
    });

    Ok(resp.into_response())
}

pub fn ws_proxy(path: BoxedFilter<()>, redirect_to: String) -> impl Filter<Extract = (warp::reply::Response,), Error = warp::Rejection> + Clone {
    path.and(ws::ws())
        .and(warp::any().map(move || redirect_to.clone()))
        .and_then(ws_proxy_handler)
}
