use async_http_codec::{BodyDecodeWithContinue, BodyEncode, RequestHead, ResponseHead};
use async_web_server::TcpIncoming;
use futures::prelude::*;
use http::header::{CONNECTION, CONTENT_LENGTH, TRANSFER_ENCODING};
use http::{Method, Request, Response, StatusCode};
use log::LevelFilter;
use simple_logger::SimpleLogger;
use smol::future::block_on;
use smol::spawn;
use std::convert::TryInto;
use std::net::Ipv6Addr;

const HTML: &[u8] = include_bytes!("echo-client.html");

fn main() -> anyhow::Result<()> {
    SimpleLogger::new()
        .with_level(LevelFilter::Info)
        .init()
        .unwrap();

    let mut incoming = TcpIncoming::bind((Ipv6Addr::UNSPECIFIED, 8080))?;

    block_on(async {
        while let Some(transport) = incoming.next().await {
            spawn(async {
                if let Err(err) = handle(transport).await {
                    log::error!("error handling request: {:?}", err);
                }
            })
            .detach();
        }
        unreachable!()
    })
}

async fn handle(mut transport: impl AsyncRead + AsyncWrite + Unpin) -> anyhow::Result<()> {
    let (_, request_head) = RequestHead::decode(&mut transport).await.unwrap();

    let mut request_body = String::new();
    BodyDecodeWithContinue::from_head(&request_head, &mut transport)?
        .read_to_string(&mut request_body)
        .await?;

    let request = Request::from_parts(request_head.into(), request_body);
    log::info!("received request: {:?}", &request);

    let mut response = Response::<&[u8]>::new(&[]);
    response
        .headers_mut()
        .insert(CONNECTION, "close".try_into()?);
    match (request.method(), request.uri().path()) {
        (&Method::GET, "/") => {
            response
                .headers_mut()
                .insert(CONTENT_LENGTH, HTML.len().into());
            *response.body_mut() = HTML;
        }
        (&Method::POST, _) => {
            response
                .headers_mut()
                .insert(TRANSFER_ENCODING, "chunked".try_into()?);
            *response.body_mut() = request.body().as_bytes();
        }
        (&Method::GET, _) => *response.status_mut() = StatusCode::NOT_FOUND,
        (_, _) => *response.status_mut() = StatusCode::METHOD_NOT_ALLOWED,
    }

    ResponseHead::ref_response(&response)
        .encode(&mut transport)
        .await?;
    let mut body_encode = BodyEncode::from_headers(&response.headers(), transport)?;
    body_encode.write_all(response.body()).await?;
    body_encode.close().await?;
    log::info!(
        "sent response with status \"{}\" on \"{}\"",
        response.status(),
        request.uri().path()
    );

    Ok(())
}
