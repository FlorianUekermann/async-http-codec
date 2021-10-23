use anyhow::bail;
use futures_lite::prelude::*;
use http::header::HeaderName;
use http::{HeaderValue, Method, Request, Uri, Version};
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct RequestHeadDecoder {
    max_head_size: usize,
    max_headers: usize,
}

impl RequestHeadDecoder {
    pub fn decode<'a, T: AsyncRead + Unpin>(
        &'a mut self,
        transport: &'a mut T,
    ) -> RequestHeadDecode<'a, T> {
        RequestHeadDecode::<'a> {
            buffer: Vec::with_capacity(self.max_head_size),
            transport,
            decoder: self,
            completion: 0,
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

pub struct RequestHeadDecode<'a, T: AsyncRead + Unpin> {
    buffer: Vec<u8>,
    transport: &'a mut T,
    decoder: &'a mut RequestHeadDecoder,
    completion: usize,
}

impl<T: AsyncRead + Unpin> Future for RequestHeadDecode<'_, T> {
    type Output = anyhow::Result<Request<()>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        assert_ne!(self.completion, usize::MAX);
        const END: &[u8; 4] = b"\r\n\r\n";
        let mut chunk = [0u8; 4];
        loop {
            let chunk = &mut chunk[self.completion..4];
            if self.buffer.len() + chunk.len() > self.buffer.capacity() {
                return Poll::Ready(Err(anyhow::Error::msg("request head too long")));
            }
            match Pin::new(&mut self.transport).poll_read(cx, chunk) {
                Poll::Ready(Ok(n)) => {
                    let chunk = &chunk[0..n];
                    self.buffer.extend_from_slice(chunk);
                    match chunk == &END[self.completion..self.completion + n] {
                        true => self.completion += n,
                        false => self.completion = 0,
                    }
                    if self.completion == END.len() {
                        self.completion = usize::MAX;
                        return Poll::Ready(request_head_parse(
                            &self.buffer,
                            self.decoder.max_headers,
                        ));
                    }
                }
                Poll::Ready(Err(err)) => {
                    self.completion = usize::MAX;
                    return Poll::Ready(Err(err.into()));
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

fn request_head_parse(buffer: &[u8], max_headers: usize) -> anyhow::Result<Request<()>> {
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
    let mut request = Request::builder()
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
    Ok(request)
}
