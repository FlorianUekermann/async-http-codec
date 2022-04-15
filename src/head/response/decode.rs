use crate::BufferHead;
use anyhow::bail;
use futures_lite::prelude::*;
use http::header::HeaderName;
use http::response::{Builder, Parts};
use http::{HeaderValue, Version};
use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Copy, Clone, Debug)]
pub struct ResponseHeadDecoder {
    max_head_size: usize,
    max_headers: usize,
}

impl ResponseHeadDecoder {
    pub fn decode<T: AsyncRead + Unpin>(self, transport: T) -> ResponseHeadDecode<T> {
        ResponseHeadDecode {
            buffer: Some(BufferHead::new(Vec::with_capacity(self.max_head_size))),
            transport: Some(transport),
            decoder: self,
        }
    }
}

impl Default for ResponseHeadDecoder {
    fn default() -> Self {
        Self {
            max_head_size: 8192,
            max_headers: 128,
        }
    }
}

pub struct ResponseHeadDecode<T: AsyncRead + Unpin> {
    buffer: Option<BufferHead>,
    transport: Option<T>,
    decoder: ResponseHeadDecoder,
}

impl<T: AsyncRead + Unpin> Future for ResponseHeadDecode<T> {
    type Output = anyhow::Result<(T, Parts)>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut transport = self.transport.take().unwrap();
        match self.buffer.as_mut().unwrap().poll(&mut transport, cx) {
            Poll::Ready(Ok(())) => Poll::Ready(
                response_head_parse(
                    &self.buffer.take().unwrap().into_inner(),
                    self.decoder.max_headers,
                )
                .map(|parts| (transport, parts)),
            ),
            Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
            Poll::Pending => {
                self.transport = Some(transport);
                Poll::Pending
            }
        }
    }
}

fn response_head_parse(buffer: &[u8], max_headers: usize) -> anyhow::Result<Parts> {
    let mut headers = vec![httparse::EMPTY_HEADER; max_headers];
    let mut parsed_response = httparse::Response::new(&mut headers);
    if parsed_response.parse(buffer)?.is_partial() {
        bail!("invalid HTTP head")
    }
    if parsed_response.version != Some(1) {
        bail!("unsupported HTTP version")
    }
    let mut response = Builder::new().version(Version::HTTP_11).body(())?;
    let headers = response.headers_mut();
    headers.reserve(parsed_response.headers.len());
    for header in parsed_response.headers {
        headers.append(
            HeaderName::from_bytes(header.name.as_bytes())?,
            HeaderValue::from_bytes(header.value)?,
        );
    }
    Ok(response.into_parts().0)
}

#[cfg(test)]
mod tests {
    use crate::ResponseHeadDecoder;
    use futures_lite::future::block_on;
    use futures_lite::io::Cursor;
    use futures_lite::{AsyncReadExt, StreamExt};
    use http::response::Parts;
    use http::{StatusCode, Version};

    const INPUT: &[u8] = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n ";

    async fn check(output: Parts, transport: Cursor<&[u8]>) {
        assert_eq!(output.version, Version::HTTP_11);
        assert_eq!(output.status, StatusCode::OK);
        assert_eq!(
            output.headers.get("Connection").unwrap().as_bytes(),
            b"close"
        );
        assert_eq!(transport.bytes().count().await, 1);
    }

    #[test]
    fn owned_transport() {
        block_on(async {
            let transport = Cursor::new(INPUT);
            let (transport, output) = ResponseHeadDecoder::default()
                .decode(transport)
                .await
                .unwrap();
            check(output, transport).await;
        })
    }

    #[test]
    fn referenced_transport() {
        block_on(async {
            let mut transport = Cursor::new(INPUT);
            let (_, output) = ResponseHeadDecoder::default()
                .decode(&mut transport)
                .await
                .unwrap();
            check(output, transport).await;
        })
    }
}
