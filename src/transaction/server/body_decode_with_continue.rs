use crate::common::length_from_headers;
use crate::internal::buffer_write::BufferWriteState;
use crate::internal::io_future::IoFutureState;
use crate::{BodyDecodeState, RequestHead, ResponseHead};
use futures::prelude::*;
use http::header::EXPECT;
use http::{HeaderMap, StatusCode, Version};
use std::borrow::{BorrowMut, Cow};
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct BodyDecodeWithContinueState {
    cont: Option<BufferWriteState>,
    flushed_cont: bool,
    body: BodyDecodeState,
}

impl BodyDecodeWithContinueState {
    pub fn from_head(head: &RequestHead) -> anyhow::Result<Self> {
        Ok(Self::from_headers(head.headers(), head.version())?)
    }
    pub fn new(version: Version, length: Option<u64>, send_continue: bool) -> Self {
        Self {
            cont: match send_continue {
                true => Some(
                    ResponseHead::new(StatusCode::CONTINUE, version, Cow::Owned(HeaderMap::new()))
                        .encode_state(),
                ),
                false => None,
            },
            flushed_cont: false,
            body: BodyDecodeState::new(length),
        }
    }
    pub fn from_headers(
        headers: &http::header::HeaderMap,
        version: Version,
    ) -> anyhow::Result<Self> {
        Ok(Self::new(
            version,
            length_from_headers(headers)?,
            contains_continue(headers),
        ))
    }
    pub fn into_async_read<IO: AsyncRead + AsyncWrite + Unpin>(
        self,
        io: IO,
    ) -> BodyDecodeWithContinue<IO> {
        BodyDecodeWithContinue { io, state: self }
    }
    pub fn poll_read<IO: AsyncRead + AsyncWrite + Unpin>(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        io: &mut IO,
    ) -> Poll<io::Result<usize>> {
        loop {
            if let Some(cont) = &mut self.cont {
                match cont.poll(cx, io) {
                    Poll::Ready(Ok(())) => self.cont.take(),
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                    Poll::Pending => return Poll::Pending,
                };
            }
            if !self.flushed_cont {
                match Pin::new(&mut *io).poll_flush(cx) {
                    Poll::Ready(Ok(())) => self.flushed_cont = true,
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                    Poll::Pending => return Poll::Pending,
                }
            }
            return self.body.poll_read(io, cx, buf);
        }
    }
}

pub struct BodyDecodeWithContinue<IO: AsyncRead + AsyncWrite + Unpin> {
    io: IO,
    state: BodyDecodeWithContinueState,
}

impl<IO: AsyncRead + AsyncWrite + Unpin> BodyDecodeWithContinue<IO> {
    pub fn from_head(head: &RequestHead, io: IO) -> anyhow::Result<Self> {
        Ok(BodyDecodeWithContinueState::from_head(head)?.into_async_read(io))
    }
    pub fn from_headers(
        headers: &http::header::HeaderMap,
        version: Version,
        io: IO,
    ) -> anyhow::Result<Self> {
        Ok(BodyDecodeWithContinueState::from_headers(headers, version)?.into_async_read(io))
    }
    pub fn new(io: IO, version: Version, length: Option<u64>, send_continue: bool) -> Self {
        BodyDecodeWithContinueState::new(version, length, send_continue).into_async_read(io)
    }
    pub fn checkpoint(self) -> (IO, BodyDecodeWithContinueState) {
        (self.io, self.state)
    }
}

impl<IO: AsyncRead + AsyncWrite + Unpin> AsyncRead for BodyDecodeWithContinue<IO> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        this.state.poll_read(cx, buf, this.io.borrow_mut())
    }
}

pub(crate) fn contains_continue(headers: &HeaderMap) -> bool {
    headers
        .get_all(EXPECT)
        .iter()
        .find(|v| v == &"100-continue")
        .is_some()
}
