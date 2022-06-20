use crate::internal::io_future::{IoFutureWithOutput, IoFutureWithOutputState};
use crate::RequestHead;
use futures::prelude::*;
use std::io;
use std::io::ErrorKind::InvalidData;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct BufferDecodeState<O: 'static> {
    buffer: Vec<u8>,
    completion: usize,
    max_headers: usize,
    decode_func: &'static (dyn Fn(&[u8], usize) -> io::Result<O> + Sync),
    _phantom: PhantomData<&'static O>,
}

#[allow(dead_code)]
const fn check_if_send<T: Send>() {}
const _: () = check_if_send::<BufferDecodeState<RequestHead>>();

impl<O> BufferDecodeState<O> {
    pub fn new(
        max_buffer: usize,
        max_headers: usize,
        decode_func: &'static (dyn Fn(&[u8], usize) -> io::Result<O> + Sync),
    ) -> Self {
        Self {
            buffer: Vec::with_capacity(max_buffer),
            completion: 0,
            max_headers,
            decode_func,
            _phantom: Default::default(),
        }
    }
}

impl<IO: AsyncRead + Unpin, O> IoFutureWithOutputState<IO, O> for BufferDecodeState<O> {
    fn poll(&mut self, cx: &mut Context<'_>, transport: &mut IO) -> Poll<io::Result<O>> {
        const END: &[u8; 4] = b"\r\n\r\n";
        let mut chunk = [0u8; END.len()];
        loop {
            let chunk = &mut chunk[self.completion..4];
            if self.buffer.len() + chunk.len() > self.buffer.capacity() {
                return Poll::Ready(Err(io::Error::new(InvalidData, "head too long")));
            }
            match Pin::new(&mut *transport).poll_read(cx, chunk) {
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
                        break;
                    }
                }
                Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                Poll::Pending => return Poll::Pending,
            }
        }
        return Poll::Ready((self.decode_func)(&self.buffer, self.max_headers));
    }
}

pub type BufferDecode<IO, O> = IoFutureWithOutput<BufferDecodeState<O>, IO, O>;
