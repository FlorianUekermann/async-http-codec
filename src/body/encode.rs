use crate::body::common::length_from_headers;
use futures::prelude::*;
use pin_project::pin_project;
use std::cmp::min;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

#[pin_project]
pub struct BodyEncode<IO: AsyncWrite + Unpin> {
    transport: IO,
    state: BodyEncodeState,
}

impl<IO: AsyncWrite + Unpin> BodyEncode<IO> {
    pub fn new(transport: IO, length: Option<u64>) -> Self {
        BodyEncodeState::new(length).restore(transport)
    }
    pub fn checkpoint(self) -> (IO, BodyEncodeState) {
        (self.transport, self.state)
    }
    pub fn from_headers(headers: &http::header::HeaderMap, transport: IO) -> anyhow::Result<Self> {
        Ok(BodyEncodeState::from_headers(headers)?.restore(transport))
    }
}

impl<IO: AsyncWrite + Unpin> AsyncWrite for BodyEncode<IO> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();
        this.state.poll_write(this.transport, cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.project();
        this.state.poll_flush(this.transport, cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.project();
        this.state.poll_close(this.transport, cx)
    }
}

pub enum BodyEncodeState {
    Fixed {
        _compression_state: (),
        remaining: u64,
    },
    Chunked(Chunked),
    Failed,
    Closed,
}

fn err_kind<T>(kind: io::ErrorKind) -> Poll<io::Result<T>> {
    Poll::Ready(Err(kind.into()))
}

impl BodyEncodeState {
    pub fn from_headers(headers: &http::header::HeaderMap) -> anyhow::Result<Self> {
        Ok(Self::new(length_from_headers(headers)?))
    }
    pub fn new(length: Option<u64>) -> Self {
        match length {
            None => Self::Chunked(Chunked {
                buffer: [0u8; 1300],
                buffered: 0,
                written: None,
                closing: false,
            }),
            Some(remaining) => Self::Fixed {
                _compression_state: (),
                remaining,
            },
        }
    }
    pub fn restore<IO: AsyncWrite + Unpin>(self, transport: IO) -> BodyEncode<IO> {
        BodyEncode {
            transport,
            state: self,
        }
    }
    fn poll_write<IO: AsyncWrite + Unpin>(
        &mut self,
        mut transport: IO,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self {
            BodyEncodeState::Fixed { remaining: 0, .. } => {
                return match buf.len() {
                    0 => Poll::Ready(Ok(0)),
                    _ => err_kind(io::ErrorKind::InvalidData),
                };
            }
            BodyEncodeState::Fixed { remaining, .. } => {
                let max_len = match (buf.len() as u64) < *remaining {
                    true => buf.len(),
                    false => *remaining as usize,
                };
                return match Pin::new(&mut transport).poll_write(cx, &buf[0..max_len]) {
                    Poll::Ready(Err(err)) => {
                        *self = BodyEncodeState::Failed;
                        Poll::Ready(Err(err))
                    }
                    Poll::Ready(Ok(n)) => {
                        *remaining -= n as u64;
                        Poll::Ready(Ok(n))
                    }
                    Poll::Pending => Poll::Pending,
                };
            }
            BodyEncodeState::Chunked(chunked) => match chunked.poll_write(transport, cx, buf) {
                Poll::Ready(Err(err)) => {
                    *self = BodyEncodeState::Failed;
                    Poll::Ready(Err(err))
                }
                p => p,
            },
            BodyEncodeState::Failed => err_kind(io::ErrorKind::BrokenPipe),
            BodyEncodeState::Closed => err_kind(io::ErrorKind::BrokenPipe),
        }
    }
    fn poll_flush<IO: AsyncWrite + Unpin>(
        &mut self,
        mut transport: IO,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        match self {
            BodyEncodeState::Fixed { .. } => match Pin::new(&mut transport).poll_flush(cx) {
                Poll::Ready(Err(err)) => {
                    *self = BodyEncodeState::Failed;
                    Poll::Ready(Err(err))
                }
                p => p,
            },
            BodyEncodeState::Chunked(chunked) => match chunked.poll_flush(transport, cx) {
                Poll::Ready(Err(err)) => {
                    *self = BodyEncodeState::Failed;
                    Poll::Ready(Err(err))
                }
                p => p,
            },
            BodyEncodeState::Failed => err_kind(io::ErrorKind::BrokenPipe),
            BodyEncodeState::Closed => err_kind(io::ErrorKind::BrokenPipe),
        }
    }
    fn poll_close<IO: AsyncWrite + Unpin>(
        &mut self,
        mut transport: IO,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        match self {
            BodyEncodeState::Fixed { .. } => match Pin::new(&mut transport).poll_close(cx) {
                Poll::Ready(Err(err)) => {
                    *self = BodyEncodeState::Failed;
                    Poll::Ready(Err(err))
                }
                Poll::Ready(Ok(())) => {
                    *self = BodyEncodeState::Closed;
                    Poll::Ready(Ok(()))
                }
                Poll::Pending => Poll::Pending,
            },
            BodyEncodeState::Chunked(chunked) => match chunked.poll_close(transport, cx) {
                Poll::Ready(Err(err)) => {
                    *self = BodyEncodeState::Failed;
                    Poll::Ready(Err(err))
                }
                Poll::Ready(Ok(())) => {
                    *self = BodyEncodeState::Closed;
                    Poll::Ready(Ok(()))
                }
                Poll::Pending => Poll::Pending,
            },
            BodyEncodeState::Failed => err_kind(io::ErrorKind::BrokenPipe),
            BodyEncodeState::Closed => Poll::Ready(Ok(())),
        }
    }
}

pub struct Chunked {
    buffer: [u8; 1300],
    buffered: usize,
    written: Option<usize>,
    closing: bool,
}

const BUFFER_HEAD: usize = 5;
const BUFFER_TAIL: usize = 2;

impl Chunked {
    fn poll_write<IO: AsyncWrite + Unpin>(
        &mut self,
        mut transport: IO,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.closing && buf.len() > 0 {
            return err_kind(io::ErrorKind::InvalidData);
        }
        let mut n = 0;
        if self.written == None {
            n += self.append(buf);
        }
        match self.poll(&mut transport, cx) {
            Poll::Pending => match n {
                0 => Poll::Pending,
                n => Poll::Ready(Ok(n)),
            },
            Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
            Poll::Ready(Ok(())) => Poll::Ready(Ok(n)),
        }
    }
    fn poll_flush<IO: AsyncWrite + Unpin>(
        &mut self,
        mut transport: IO,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        if self.buffered > 0 && self.written == None {
            self.finalize_chunk();
        }
        while self.written != None {
            match self.poll(Pin::new(&mut transport), cx) {
                Poll::Ready(Ok(())) => {}
                p => return p,
            }
        }
        Pin::new(&mut transport).poll_flush(cx)
    }
    fn poll_close<IO: AsyncWrite + Unpin>(
        &mut self,
        mut transport: IO,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        while self.written != None || !self.closing {
            if self.written == None {
                if self.buffered == 0 {
                    self.closing = true;
                }
                self.finalize_chunk();
            }
            match self.poll(Pin::new(&mut transport), cx) {
                Poll::Ready(Ok(())) => {}
                p => return p,
            }
        }
        Pin::new(&mut transport).poll_close(cx)
    }
    fn append(&mut self, buf: &[u8]) -> usize {
        let off = BUFFER_HEAD + self.buffered;
        let n = min(buf.len(), self.buffer.len() - off - BUFFER_TAIL);
        self.buffer[off..off + n].copy_from_slice(&buf[0..n]);
        self.buffered += n;
        if self.buffered + BUFFER_TAIL + BUFFER_HEAD == self.buffer.len() {
            self.finalize_chunk();
        }
        n
    }
    fn finalize_chunk(&mut self) {
        self.buffer[BUFFER_HEAD - 2..BUFFER_HEAD].copy_from_slice(b"\r\n");
        let end = BUFFER_HEAD + self.buffered + BUFFER_TAIL;
        self.buffer[end - 2..end].copy_from_slice(b"\r\n");
        let mut len = self.buffered;
        let mut start = BUFFER_HEAD - 2;
        while len > 0 || start == BUFFER_HEAD - 2 {
            let digit = len & 15;
            len /= 16;
            start -= 1;
            self.buffer[start] = match digit {
                0..=9 => b'0' + digit as u8,
                10..=15 => b'A' - 10 + digit as u8,
                _ => unreachable!(),
            };
        }
        self.written = Some(start);
    }
    fn poll<IO: AsyncWrite + Unpin>(
        &mut self,
        mut transport: IO,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        match self.written {
            None => Poll::Ready(Ok(())),
            Some(written) => {
                let end = BUFFER_HEAD + self.buffered + BUFFER_TAIL;
                match Pin::new(&mut transport).poll_write(cx, &self.buffer[written..end]) {
                    Poll::Ready(Ok(n)) => {
                        self.written = Some(written + n);
                        if self.written == Some(end) {
                            self.buffered = 0;
                            self.written = None;
                        }
                        Poll::Ready(Ok(()))
                    }
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
                }
            }
        }
    }
}
