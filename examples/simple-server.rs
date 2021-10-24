use async_http_codec::head::decode::RequestHeadDecoder;
use async_http_codec::head::encode::ResponseHeadEncoder;
use async_net_server_utils::tcp::TcpIncoming;
use futures_lite::future::block_on;
use futures_lite::prelude::*;
use http::{HeaderValue, Response};
use log::LevelFilter;
use simple_logger::SimpleLogger;
use std::net::Ipv4Addr;

fn main() -> anyhow::Result<()> {
    SimpleLogger::new()
        .with_level(LevelFilter::Info)
        .init()
        .unwrap();
    block_on(async {
        let mut incoming = TcpIncoming::bind((Ipv4Addr::UNSPECIFIED, 8080))?;
        while let Some(mut transport) = incoming.next().await {
            let (_, request) = RequestHeadDecoder::default()
                .decode_ref(&mut transport)
                .await
                .unwrap();
            log::info!("{:?}", &request);

            let response = Response::builder()
                .header("Content-Length", HeaderValue::from(6))
                .body(())
                .unwrap();
            ResponseHeadEncoder::default()
                .encode_ref(&mut transport, response)
                .await
                .unwrap();
            transport.write_all(b"hello\n").await.unwrap();
        }

        Ok(())
    })
}
