use crate::common::ERROR;
use anyhow::Error;
use futures::{SinkExt, StreamExt};
use std::str::FromStr;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;
use warp::filters::BoxedFilter;
use warp::http::{Request, Uri};
use warp::reject::Reject;
use warp::ws::{self, Message as WarpMessage};
use warp::{http, reject, Filter, Rejection, Reply};

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

async fn http_proxy_handler(mut request: Request<warp::hyper::Body>, proxy_to: String) -> Result<warp::reply::Response, Rejection> {
    let client = warp::hyper::client::Client::new();
    let uri = request.uri();
    let proxy_to = proxy_to.strip_suffix("/").unwrap_or(&proxy_to);

    *request.uri_mut() = Uri::from_str(&format!("{}{}", proxy_to, uri)).unwrap();

    client.request(request).await.map_err(|e| reject::custom(ProxyRejection(Error::from(e))))
}

pub fn http_proxy(path: String, proxy_to: String) -> impl Filter<Extract = (warp::reply::Response,), Error = warp::Rejection> + Clone {
    if path.is_empty() {
        warp::any().boxed()
    } else {
        warp::path(path).boxed()
    }
    .and(extract_request())
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
            while let Some(Ok(item)) = remote_source.next().await {
                let msg = if item.is_binary() {
                    WarpMessage::binary(item.into_data())
                } else if item.is_text() {
                    WarpMessage::text(item.into_text().unwrap())
                } else if item.is_close() {
                    if let TungsteniteMessage::Close(Some(frame)) = item {
                        WarpMessage::close_with(frame.code, frame.reason)
                    } else {
                        WarpMessage::close()
                    }
                } else if item.is_ping() {
                    WarpMessage::ping(item.into_data())
                } else {
                    unimplemented!("unavailable message")
                };

                if warp_sink.send(msg).await.is_ok() {
                    warp_sink.flush().await.unwrap();
                }
            }
        };
        let redirect_remote = async move {
            while let Some(Ok(item)) = warp_source.next().await {
                let msg = if item.is_binary() {
                    TungsteniteMessage::binary(item.into_bytes())
                } else if item.is_text() {
                    TungsteniteMessage::text(item.to_str().unwrap())
                } else if item.is_close() {
                    TungsteniteMessage::Close(None) // todo
                } else if item.is_ping() {
                    TungsteniteMessage::Ping(item.into_bytes())
                } else if item.is_pong() {
                    TungsteniteMessage::Pong(item.into_bytes())
                } else {
                    unimplemented!("unavailable message")
                };

                if remote_sink.send(msg).await.is_ok() {
                    remote_sink.flush().await.unwrap();
                }
            }
        };

        let handle1 = tokio::spawn(redirect_warp);
        let handle2 = tokio::spawn(redirect_remote);

        if let Err(e) = tokio::try_join!(handle1, handle2) {
            eprintln!("{} websocket proxy error: {}", ERROR, e)
        };
    });

    Ok(resp.into_response())
}

pub fn ws_proxy(path: BoxedFilter<()>, redirect_to: String) -> impl Filter<Extract = (warp::reply::Response,), Error = warp::Rejection> + Clone {
    path.and(ws::ws())
        .and(warp::any().map(move || redirect_to.clone()))
        .and_then(ws_proxy_handler)
}
