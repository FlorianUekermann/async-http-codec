use anyhow::bail;
use futures_lite::prelude::*;
use http::Response;
use std::borrow::BorrowMut;
use std::io::Write;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

#[derive(Copy, Clone, Debug)]
pub struct ResponseHeadEncoder {}

impl ResponseHeadEncoder {
    pub fn encode<T: AsyncWrite + Unpin, R: BorrowMut<T>>(
        self,
        transport: R,
        response: Response<()>,
    ) -> ResponseHeadEncode<T, R> {
        ResponseHeadEncode {
            transport,
            buffer: None,
            _encoder: self,
            completion: 0,
            response,
            p: Default::default(),
        }
    }
    pub fn encode_ref<T: AsyncWrite + Unpin>(
        self,
        transport: &mut T,
        response: Response<()>,
    ) -> ResponseHeadEncode<T, &mut T> {
        self.encode(transport, response)
    }
}

impl Default for ResponseHeadEncoder {
    fn default() -> Self {
        Self {}
    }
}

#[pin_project::pin_project]
pub struct ResponseHeadEncode<T: AsyncWrite + Unpin, R: BorrowMut<T>> {
    transport: R,
    _encoder: ResponseHeadEncoder,
    response: Response<()>,
    buffer: Option<Arc<Vec<u8>>>,
    completion: usize,
    p: PhantomData<*const T>,
}

impl<T: AsyncWrite + Unpin, R: BorrowMut<T>> Future for ResponseHeadEncode<T, R> {
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
            match Pin::new(self.transport.borrow_mut()).poll_write(cx, remainder) {
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
