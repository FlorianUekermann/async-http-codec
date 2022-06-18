use crate::{IoFuture, IoFutureState};
use futures::AsyncWrite;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

pub(crate) struct BufferWriteState {
    buffer: Vec<u8>,
    completion: usize,
}

impl BufferWriteState {
    pub fn new(buffer: Vec<u8>) -> Self {
        Self {
            buffer,
            completion: 0,
        }
    }
}

impl<IO: AsyncWrite + Unpin> IoFutureState<IO> for BufferWriteState {
    fn poll(&mut self, cx: &mut Context<'_>, io: &mut IO) -> Poll<io::Result<()>> {
        loop {
            let remainder = &self.buffer[self.completion..];
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

pub(crate) type BufferWrite<IO> = IoFuture<IO, BufferWriteState>;

#[cfg(test)]
mod tests {
    use crate::buffer_write::{BufferWrite, BufferWriteState};
    use crate::IoFutureState;
    use futures::executor::block_on;
    use futures::io::Cursor;

    #[test]
    fn test() {
        block_on(async {
            const HELLO_WORLD: &[u8] = b"Hello World!";
            let mut io = Cursor::new(Vec::new());
            let fut = BufferWriteState::new(HELLO_WORLD.to_vec()).into_future(&mut io);
            fut.await.unwrap();

            assert_eq!(
                String::from_utf8(HELLO_WORLD.to_vec()).unwrap(),
                String::from_utf8(io.into_inner()).unwrap()
            );
        })
    }
}

// impl<IO: AsyncWrite + Unpin> Future for BufferWrite<IO> {
//     type Output = io::Result<IO>;
//     fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
//         state_poll(&mut self.0, cx)
//     }
//     // fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
//     //     let (mut state, mut io) = self.0.take().unwrap();
//     //     let p = state.poll(cx, &mut io);
//     //     self.0 = Some((state, io));
//     //     p.map(|r| r.map(|()| self.0.take().unwrap().1))
//     // }
// }
//
// fn state_poll<S: IoFutureState<IO>, IO>(o: &mut Option<(S, IO)>, cx: &mut Context<'_>) -> Poll<io::Result<IO>> {
//     let (mut state, mut io) = o.take().unwrap();
//     let p = state.poll(cx, &mut io);
//     *o = Some((state, io));
//     p.map(|r| r.map(|()| o.take().unwrap().1))
// }
//
pub(crate) fn write_buffer<IO: AsyncWrite + Unpin>(
    io: &mut IO,
    buffer: &[u8],
    completion: &mut usize,
    cx: &mut Context<'_>,
) -> Poll<io::Result<()>> {
    loop {
        let remainder = &buffer[*completion..];
        match Pin::new(&mut *io).poll_write(cx, remainder) {
            Poll::Ready(Ok(n)) => {
                if n == remainder.len() {
                    return Poll::Ready(Ok(()));
                }
                *completion += n;
            }
            Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
            Poll::Pending => {
                return Poll::Pending;
            }
        }
    }
}
