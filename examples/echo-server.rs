use async_http_codec::{BodyDecodeState, RequestHeadDecoder};
use async_http_codec::{BodyEncodeState, ResponseHeadEncoder};
use async_net_server_utils::tcp::TcpIncoming;
use futures::executor::block_on;
use futures::prelude::*;
use http::header::{CONNECTION, CONTENT_LENGTH, TRANSFER_ENCODING};
use http::{HeaderValue, Method, Response, StatusCode};
use log::LevelFilter;
use simple_logger::SimpleLogger;
use std::convert::TryInto;
use std::net::Ipv4Addr;

const HTML: &[u8] = include_bytes!("echo-client.html");

fn main() -> anyhow::Result<()> {
    SimpleLogger::new()
        .with_level(LevelFilter::Info)
        .init()
        .unwrap();

    let mut incoming = TcpIncoming::bind((Ipv4Addr::UNSPECIFIED, 8080))?;

    block_on(async {
        while let Some(transport) = incoming.next().await {
            if let Err(err) = handle(transport).await {
                log::error!("error handling request: {:?}", err);
            }
        }
        unreachable!()
    })
}

async fn handle(mut transport: impl AsyncRead + AsyncWrite + Unpin) -> anyhow::Result<()> {
    let (_, request_head) = RequestHeadDecoder::default()
        .decode(&mut transport)
        .await
        .unwrap();
    log::info!("received request head: {:?}", request_head.method);

    let mut request_body = String::new();
    BodyDecodeState::from_headers(&request_head.headers)?
        .restore(&mut transport)
        .read_to_string(&mut request_body)
        .await?;
    log::info!("received request body: {:?}", request_body);

    let mut response_head = Response::new(()).into_parts().0;
    response_head
        .headers
        .insert(CONNECTION, "close".try_into()?);
    match request_head.method {
        Method::GET => {
            response_head
                .headers
                .insert(CONTENT_LENGTH, HeaderValue::from(HTML.len()));
        }
        Method::POST => {
            response_head
                .headers
                .insert(TRANSFER_ENCODING, "chunked".try_into()?);
        }
        _ => {
            response_head.status = StatusCode::METHOD_NOT_ALLOWED;
            response_head
                .headers
                .insert(CONTENT_LENGTH, HeaderValue::from(0));
        }
    }
    ResponseHeadEncoder::default()
        .encode(&mut transport, &response_head)
        .await?;
    log::info!("sent response head: {:?}", &response_head);

    let mut body_encode = BodyEncodeState::from_headers(&response_head.headers)?.restore(transport);
    match request_head.method {
        Method::GET => body_encode.write_all(HTML).await?,
        Method::POST => body_encode.write_all(request_body.as_bytes()).await?,
        _ => {}
    }
    body_encode.close().await?;
    log::info!("sent response body");

    Ok(())
}
