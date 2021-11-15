use anyhow::bail;
use futures_lite::prelude::*;
use http::Response;
use std::io::Write;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

#[derive(Copy, Clone, Debug)]
pub struct ResponseHeadEncoder {}

impl ResponseHeadEncoder {
    pub fn encode<T: AsyncWrite + Unpin>(
        self,
        transport: T,
        response: Response<()>,
    ) -> ResponseHeadEncode<T> {
        ResponseHeadEncode {
            transport: Some(transport),
            buffer: Arc::new(Vec::new()),
            _encoder: self,
            completion: 0,
            response,
        }
    }
}

impl Default for ResponseHeadEncoder {
    fn default() -> Self {
        Self {}
    }
}

pub struct ResponseHeadEncode<T: AsyncWrite + Unpin> {
    transport: Option<T>,
    _encoder: ResponseHeadEncoder,
    response: Response<()>,
    buffer: Arc<Vec<u8>>,
    completion: usize,
}

impl<T: AsyncWrite + Unpin> Future for ResponseHeadEncode<T> {
    type Output = anyhow::Result<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut transport = self.transport.take().unwrap();
        if self.buffer.is_empty() {
            match response_head_encode(&self.response) {
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
                .encode(&mut transport, Response::new(()))
                .await
                .unwrap();
            assert_eq!(transport.into_inner(), OUTPUT);
        })
    }
}
