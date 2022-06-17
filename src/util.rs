use futures::AsyncWrite;
use std::pin::Pin;
use std::task::{Context, Poll};

pub(crate) fn write_buffer<T: AsyncWrite + Unpin>(
    transport: &mut T,
    buffer: &[u8],
    completion: &mut usize,
    cx: &mut Context<'_>,
) -> Poll<Result<(), std::io::Error>> {
    loop {
        let remainder = &buffer[*completion..];
        match Pin::new(&mut *transport).poll_write(cx, remainder) {
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
