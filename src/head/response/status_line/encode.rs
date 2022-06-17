use crate::util::write_buffer;
use futures::AsyncWrite;
use http::{StatusCode, Version};
use pin_project::pin_project;
use std::future::Future;
use std::io::Write;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

#[derive(Copy, Clone, Debug)]
pub struct StatusLineEncoder {}

impl StatusLineEncoder {
    pub fn encode<T: AsyncWrite + Unpin>(
        self,
        transport: T,
        version: Version,
        code: StatusCode,
    ) -> StatusLineEncode<T> {
        StatusLineEncode {
            version,
            code,
            transport: Some(transport),
            buffer: Arc::new(Vec::new()),
            _encoder: self,
            completion: 0,
        }
    }
}

impl Default for StatusLineEncoder {
    fn default() -> Self {
        Self {}
    }
}

#[pin_project]
pub struct StatusLineEncode<T: AsyncWrite + Unpin> {
    version: Version,
    code: StatusCode,
    transport: Option<T>,
    _encoder: StatusLineEncoder,
    buffer: Arc<Vec<u8>>,
    completion: usize,
}

impl<T: AsyncWrite + Unpin> Future for StatusLineEncode<T> {
    type Output = anyhow::Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        let mut transport = this.transport.take().unwrap();
        if this.buffer.is_empty() {
            match informational_response_encode(this.version, this.code) {
                Ok(buffer) => *this.buffer = Arc::new(buffer),
                Err(err) => return Poll::Ready(Err(err)),
            }
        }

        match write_buffer(&mut transport, &this.buffer, &mut this.completion, cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(transport)),
            Poll::Ready(Err(err)) => Poll::Ready(Err(err.into())),
            Poll::Pending => {
                *this.transport = Some(transport);
                Poll::Pending
            }
        }
    }
}

fn informational_response_encode(
    version: &Version,
    status: &StatusCode,
) -> anyhow::Result<Vec<u8>> {
    let mut buffer = Vec::with_capacity(1024);
    writeln!(buffer, "{:?} {}\r", version, status)?;
    Ok(buffer)
}
