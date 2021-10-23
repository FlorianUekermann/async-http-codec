use anyhow::bail;
use futures_lite::prelude::*;
use http::Response;
use std::io::Write;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

pub struct RequestHeadEncoder {}

impl RequestHeadEncoder {
    pub fn decode<'a, T: AsyncWrite + Unpin>(
        &'a mut self,
        transport: &'a mut T,
        response: Response<()>,
    ) -> RequestHeadEncode<'a, T> {
        RequestHeadEncode::<'a> {
            transport,
            buffer: None,
            _encoder: self,
            completion: 0,
            response,
        }
    }
}

impl Default for RequestHeadEncoder {
    fn default() -> Self {
        Self {}
    }
}

pub struct RequestHeadEncode<'a, T: AsyncWrite + Unpin> {
    transport: &'a mut T,
    _encoder: &'a RequestHeadEncoder,
    response: Response<()>,
    buffer: Option<Arc<Vec<u8>>>,
    completion: usize,
}

impl<T: AsyncWrite + Unpin> Future for RequestHeadEncode<'_, T> {
    type Output = anyhow::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        assert_ne!(self.completion, usize::MAX);
        if self.buffer.is_none() {
            match response_head_encode(&self.response) {
                Ok(buffer) => self.buffer = Some(Arc::new(buffer)),
                Err(err) => {
                    self.completion = usize::MAX;
                    return Poll::Ready(Err(err));
                }
            }
        }
        let buffer = self.buffer.as_ref().unwrap().clone();
        loop {
            let remainder = &buffer[self.completion..];
            match Pin::new(&mut self.transport).poll_write(cx, remainder) {
                Poll::Ready(Ok(n)) => {
                    if n == remainder.len() {
                        self.completion = usize::MAX;
                        return Poll::Ready(Ok(remainder.len() + n));
                    }
                    self.completion += n;
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

fn response_head_encode(response: &Response<()>) -> anyhow::Result<Vec<u8>> {
    let mut buffer = Vec::with_capacity(8192);
    writeln!(buffer, "{:?} {}\r", response.version(), response.status())?;
    for (k, v) in response.headers() {
        let v = match v.to_str() {
            Err(_) => bail!("invalid character in header value"),
            Ok(v) => v,
        };
        writeln!(buffer, "{}: {}\r", k, v)?;
    }
    writeln!(buffer, "\r")?;
    Ok(buffer)
}
