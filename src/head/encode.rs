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
            transport: Some(transport),
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
    transport: Option<R>,
    _encoder: ResponseHeadEncoder,
    response: Response<()>,
    buffer: Option<Arc<Vec<u8>>>,
    completion: usize,
    p: PhantomData<*const T>,
}

impl<T: AsyncWrite + Unpin, R: BorrowMut<T>> Future for ResponseHeadEncode<T, R> {
    type Output = anyhow::Result<R>;

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
            let transport = Pin::new(self.transport.as_mut().unwrap().borrow_mut());
            match transport.poll_write(cx, remainder) {
                Poll::Ready(Ok(n)) => {
                    if n == remainder.len() {
                        self.completion = usize::MAX;
                        return Poll::Ready(Ok(self.transport.take().unwrap()));
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

#[cfg(test)]
mod tests {
    use crate::head::encode::ResponseHeadEncoder;
    use futures_lite::future::block_on;
    use futures_lite::io::Cursor;
    use http::Response;

    const OUTPUT: &[u8] = b"HTTP/1.1 200 OK\r\n\r\n";

    #[test]
    fn owned_transport() {
        block_on(async {
            let transport = Cursor::new(Vec::new());
            let transport = ResponseHeadEncoder::default()
                .encode(transport, Response::new(()))
                .await
                .unwrap();
            assert_eq!(transport.into_inner(), OUTPUT);
        })
    }

    #[test]
    fn referenced_transport() {
        block_on(async {
            let mut transport = Cursor::new(Vec::new());
            ResponseHeadEncoder::default()
                .encode_ref(&mut transport, Response::new(()))
                .await
                .unwrap();
            assert_eq!(transport.into_inner(), OUTPUT);
        })
    }
}
