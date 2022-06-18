use crate::buffer_write::write_buffer;
use futures::AsyncWrite;
use http::{StatusCode, Version};
use pin_project::pin_project;
use std::future::Future;
use std::io;
use std::io::Write;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

#[pin_project]
pub struct StatusLineEncode<T: AsyncWrite + Unpin> {
    version: Version,
    code: StatusCode,
    transport: Option<T>,
    buffer: Arc<Vec<u8>>,
    completion: usize,
}

impl<T: AsyncWrite + Unpin> StatusLineEncode<T> {
    pub fn new(transport: T, version: Version, code: StatusCode) -> StatusLineEncode<T> {
        StatusLineEncode {
            version,
            code,
            transport: Some(transport),
            buffer: Arc::new(Vec::new()),
            completion: 0,
        }
    }
}

impl<T: AsyncWrite + Unpin> Future for StatusLineEncode<T> {
    type Output = io::Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        let mut transport = this.transport.take().unwrap();
        if this.buffer.is_empty() {
            let mut buffer = Vec::with_capacity(1024);
            match status_line_encode(&mut buffer, this.version, this.code) {
                Ok(()) => *this.buffer = Arc::new(buffer),
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
pub(crate) fn status_line_encode(
    buffer: &mut Vec<u8>,
    version: &Version,
    status: &StatusCode,
) -> io::Result<()> {
    writeln!(buffer, "{:?} {}\r", version, status)?;
    Ok(())
}
