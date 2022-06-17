use crate::head::common::header_encode;
use crate::util::write_buffer;
use futures::prelude::*;
use http::request::Parts;
use pin_project::pin_project;
use std::borrow::Borrow;
use std::io::Write;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

#[derive(Copy, Clone, Debug)]
pub struct RequestHeadEncoder {}

impl RequestHeadEncoder {
    pub fn encode<T: AsyncWrite + Unpin, P: Borrow<Parts>>(
        self,
        transport: T,
        head: P,
    ) -> RequestHeadEncode<T, P> {
        RequestHeadEncode {
            transport_head: Some((transport, head)),
            buffer: Arc::new(Vec::new()),
            _encoder: self,
            completion: 0,
        }
    }
}

impl Default for RequestHeadEncoder {
    fn default() -> Self {
        Self {}
    }
}

#[pin_project]
pub struct RequestHeadEncode<T: AsyncWrite + Unpin, P: Borrow<Parts>> {
    transport_head: Option<(T, P)>,
    _encoder: RequestHeadEncoder,
    buffer: Arc<Vec<u8>>,
    completion: usize,
}

impl<T: AsyncWrite + Unpin, P: Borrow<Parts>> Future for RequestHeadEncode<T, P> {
    type Output = anyhow::Result<(T, P)>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        let (mut transport, head) = this.transport_head.take().unwrap();
        if this.buffer.is_empty() {
            match request_head_encode(head.borrow()) {
                Ok(buffer) => *this.buffer = Arc::new(buffer),
                Err(err) => return Poll::Ready(Err(err)),
            }
        }

        match write_buffer(&mut transport, &this.buffer, &mut this.completion, cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok((transport, head))),
            Poll::Ready(Err(err)) => Poll::Ready(Err(err.into())),
            Poll::Pending => {
                *this.transport_head = Some((transport, head));
                Poll::Pending
            }
        }
    }
}

fn request_head_encode(head: &Parts) -> anyhow::Result<Vec<u8>> {
    let mut buffer = Vec::with_capacity(8192);
    writeln!(buffer, "{} {} {:?}\r", head.method, head.uri, head.version)?;
    header_encode(&mut buffer, &head.headers)?;
    Ok(buffer)
}
