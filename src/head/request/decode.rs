use crate::BufferHead;
use anyhow::bail;
use futures_lite::prelude::*;
use http::header::HeaderName;
use http::request::{Builder, Parts};
use http::{HeaderValue, Method, Uri, Version};
use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Copy, Clone, Debug)]
pub struct RequestHeadDecoder {
    max_head_size: usize,
    max_headers: usize,
}

impl RequestHeadDecoder {
    pub fn decode<T: AsyncRead + Unpin>(self, transport: T) -> RequestHeadDecode<T> {
        RequestHeadDecode {
            buffer: Some(BufferHead::new(Vec::with_capacity(self.max_head_size))),
            transport: Some(transport),
            decoder: self,
        }
    }
}

impl Default for RequestHeadDecoder {
    fn default() -> Self {
        Self {
            max_head_size: 8192,
            max_headers: 128,
        }
    }
}

pub struct RequestHeadDecode<T: AsyncRead + Unpin> {
    buffer: Option<BufferHead>,
    transport: Option<T>,
    decoder: RequestHeadDecoder,
}

impl<T: AsyncRead + Unpin> Future for RequestHeadDecode<T> {
    type Output = anyhow::Result<(T, Parts)>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut transport = self.transport.take().unwrap();
        match self.buffer.as_mut().unwrap().poll(&mut transport, cx) {
            Poll::Ready(Ok(())) => Poll::Ready(
                request_head_parse(
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

fn request_head_parse(buffer: &[u8], max_headers: usize) -> anyhow::Result<Parts> {
    let mut headers = vec![httparse::EMPTY_HEADER; max_headers];
    let mut parsed_request = httparse::Request::new(&mut headers);
    if parsed_request.parse(buffer)?.is_partial() {
        bail!("invalid HTTP head")
    }
    if parsed_request.version != Some(1) {
        bail!("unsupported HTTP version")
    }
    let method = Method::from_bytes(parsed_request.method.unwrap_or("").as_bytes())?;
    let uri = parsed_request.path.unwrap_or("").parse::<Uri>()?;
    let mut request = Builder::new()
        .method(method)
        .uri(uri)
        .version(Version::HTTP_11)
        .body(())?;
    let headers = request.headers_mut();
    headers.reserve(parsed_request.headers.len());
    for header in parsed_request.headers {
        headers.append(
            HeaderName::from_bytes(header.name.as_bytes())?,
            HeaderValue::from_bytes(header.value)?,
        );
    }
    Ok(request.into_parts().0)
}
