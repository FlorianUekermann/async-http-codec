use anyhow::bail;
use futures_lite::prelude::*;
use http::request::Parts;
use std::io::Write;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

#[derive(Copy, Clone, Debug)]
pub struct RequestHeadEncoder {}

impl RequestHeadEncoder {
    pub fn encode<T: AsyncWrite + Unpin>(self, transport: T, head: Parts) -> RequestHeadEncode<T> {
        RequestHeadEncode {
            transport: Some(transport),
            buffer: Arc::new(Vec::new()),
            _encoder: self,
            completion: 0,
            head,
        }
    }
}

impl Default for RequestHeadEncoder {
    fn default() -> Self {
        Self {}
    }
}

pub struct RequestHeadEncode<T: AsyncWrite + Unpin> {
    transport: Option<T>,
    _encoder: RequestHeadEncoder,
    head: Parts,
    buffer: Arc<Vec<u8>>,
    completion: usize,
}

impl<T: AsyncWrite + Unpin> Future for RequestHeadEncode<T> {
    type Output = anyhow::Result<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut transport = self.transport.take().unwrap();
        if self.buffer.is_empty() {
            match request_head_encode(&self.head) {
                Ok(buffer) => self.buffer = Arc::new(buffer),
                Err(err) => return Poll::Ready(Err(err)),
            }
        }

        loop {
            let remainder = &self.buffer[self.completion..];
            match Pin::new(&mut transport).poll_write(cx, remainder) {
                Poll::Ready(Ok(n)) => {
                    if n == remainder.len() {
                        return Poll::Ready(Ok(transport));
                    }
                    self.completion += n;
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

fn request_head_encode(head: &Parts) -> anyhow::Result<Vec<u8>> {
    let mut buffer = Vec::with_capacity(8192);
    writeln!(buffer, "{} {} {:?}\r", head.method, head.uri, head.version)?;
    for (k, v) in &head.headers {
        let v = match v.to_str() {
            Err(_) => bail!("invalid character in header value"),
            Ok(v) => v,
        };
        writeln!(buffer, "{}: {}\r", k, v)?;
    }
    writeln!(buffer, "\r")?;
    Ok(buffer)
}
