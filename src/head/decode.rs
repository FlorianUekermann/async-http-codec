use anyhow::bail;
use futures_lite::prelude::*;
use http::header::HeaderName;
use http::{HeaderValue, Method, Request, Uri, Version};
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
            buffer: Vec::with_capacity(self.max_head_size),
            transport: Some(transport),
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

pub struct RequestHeadDecode<T: AsyncRead + Unpin> {
    buffer: Vec<u8>,
    transport: Option<T>,
    decoder: RequestHeadDecoder,
    completion: usize,
}

impl<T: AsyncRead + Unpin> Future for RequestHeadDecode<T> {
    type Output = anyhow::Result<(T, Request<()>)>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut transport = self.transport.take().unwrap();
        const END: &[u8; 4] = b"\r\n\r\n";
        let mut chunk = [0u8; END.len()];
        loop {
            let chunk = &mut chunk[self.completion..4];
            if self.buffer.len() + chunk.len() > self.buffer.capacity() {
                return Poll::Ready(Err(anyhow::Error::msg("request head too long")));
            }
            match Pin::new(&mut transport).poll_read(cx, chunk) {
                Poll::Ready(Ok(n)) => {
                    let mut chunk = &chunk[0..n];
                    self.buffer.extend_from_slice(chunk);
                    while self.completion == 0 && chunk.len() > 0 {
                        if chunk[0] == END[0] {
                            self.completion = 1
                        }
                        chunk = &chunk[1..];
                    }
                    match chunk == &END[self.completion..self.completion + chunk.len()] {
                        true => self.completion += chunk.len(),
                        false => self.completion = 0,
                    }
                    if self.completion == END.len() {
                        return Poll::Ready(
                            request_head_parse(&self.buffer, self.decoder.max_headers)
                                .map(|request| (transport, request)),
                        );
                    }
                }
                Poll::Ready(Err(err)) => return Poll::Ready(Err(err.into())),
                Poll::Pending => {
                    self.transport = Some(transport);
                    return Poll::Pending;
                }
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

#[cfg(test)]
mod tests {
    use crate::head::decode::RequestHeadDecoder;
    use futures_lite::future::block_on;
    use futures_lite::io::Cursor;
    use futures_lite::{AsyncReadExt, StreamExt};
    use http::{Method, Request, Version};

    const INPUT: &[u8] = b"GET / HTTP/1.1\r\nHost: www.example.com\r\nConnection: close\r\n\r\n ";

    async fn check(output: Request<()>, transport: Cursor<&[u8]>) {
        assert_eq!(output.version(), Version::HTTP_11);
        assert_eq!(output.method(), Method::GET);
        assert_eq!(
            output.headers().get("Host").unwrap().as_bytes(),
            b"www.example.com"
        );
        assert_eq!(
            output.headers().get("Connection").unwrap().as_bytes(),
            b"close"
        );
        assert_eq!(transport.bytes().count().await, 1);
    }

    #[test]
    fn owned_transport() {
        block_on(async {
            let transport = Cursor::new(INPUT);
            let (transport, output) = RequestHeadDecoder::default()
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
            let (_, output) = RequestHeadDecoder::default()
                .decode(&mut transport)
                .await
                .unwrap();
            check(output, transport).await;
        })
    }
}
