use crate::internal::io_future::{IoFuture, IoFutureState};
use futures::AsyncWrite;
use std::io;
use std::mem::replace;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct BufferWriteState {
    buffer: io::Result<Vec<u8>>,
    completion: usize,
}

impl BufferWriteState {
    pub fn new(buffer: io::Result<Vec<u8>>) -> Self {
        Self {
            buffer,
            completion: 0,
        }
    }
}

impl<IO: AsyncWrite + Unpin> IoFutureState<IO> for BufferWriteState {
    fn poll(&mut self, cx: &mut Context<'_>, io: &mut IO) -> Poll<io::Result<()>> {
        let buffer = match &self.buffer {
            Ok(buffer) => buffer,
            Err(_) => {
                let r = replace(&mut self.buffer, Ok(Vec::new()));
                return Poll::Ready(Err(r.unwrap_err()));
            }
        };
        loop {
            let remainder = &buffer[self.completion..];
            match Pin::new(&mut *io).poll_write(cx, &remainder) {
                Poll::Ready(Ok(n)) => {
                    if n == remainder.len() {
                        return Poll::Ready(Ok(()));
                    }
                    self.completion += n;
                }
                Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

pub type BufferWrite<IO> = IoFuture<BufferWriteState, IO>;

#[cfg(test)]
mod tests {
    use crate::internal::buffer_write::{BufferWrite, BufferWriteState};
    use crate::internal::io_future::IoFutureState;
    use futures::executor::block_on;
    use futures::io::Cursor;

    #[test]
    fn test() {
        block_on(async {
            const HELLO_WORLD: &[u8] = b"Hello World!";
            let mut io = Cursor::new(Vec::new());
            let fut: BufferWrite<_> =
                BufferWriteState::new(Ok(HELLO_WORLD.to_vec())).into_future(&mut io);
            fut.await.unwrap();

            assert_eq!(
                String::from_utf8(HELLO_WORLD.to_vec()).unwrap(),
                String::from_utf8(io.into_inner()).unwrap()
            );
        })
    }
}
