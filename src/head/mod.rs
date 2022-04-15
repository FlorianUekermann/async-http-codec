mod request;
mod response;

use futures_lite::prelude::*;
pub use request::*;
pub use response::*;
use std::pin::Pin;
use std::task::{Context, Poll};

pub(crate) struct BufferHead {
    buffer: Vec<u8>,
    completion: usize,
}

impl BufferHead {
    fn new(buffer: Vec<u8>) -> Self {
        Self {
            buffer,
            completion: 0,
        }
    }
    fn into_inner(self) -> Vec<u8> {
        self.buffer
    }
    fn poll<T: AsyncRead + Unpin>(
        &mut self,
        mut transport: T,
        cx: &mut Context<'_>,
    ) -> Poll<anyhow::Result<()>> {
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
                        return Poll::Ready(Ok(()));
                    }
                }
                Poll::Ready(Err(err)) => return Poll::Ready(Err(err.into())),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}
