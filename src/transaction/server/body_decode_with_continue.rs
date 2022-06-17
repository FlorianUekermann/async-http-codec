use crate::{length_from_headers, BodyDecode, StatusLineEncode};
use futures::prelude::*;
use http::header::EXPECT;
use http::{HeaderMap, StatusCode, Version};
use pin_project::pin_project;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

#[pin_project]
pub struct BodyDecodeWithContinue<IO: AsyncRead + AsyncWrite + Unpin> {
    length: Option<u64>,
    cont: Option<StatusLineEncode<IO>>,
    body: Option<BodyDecode<IO>>,
}

impl<IO: AsyncRead + AsyncWrite + Unpin> BodyDecodeWithContinue<IO> {
    pub fn from_headers(
        headers: &http::header::HeaderMap,
        version: Version,
        transport: IO,
    ) -> anyhow::Result<Self> {
        Ok(Self::new(
            transport,
            version,
            length_from_headers(headers)?,
            contains_continue(headers),
        ))
    }
    pub fn new(transport: IO, version: Version, length: Option<u64>, send_continue: bool) -> Self {
        if send_continue {
            Self {
                length,
                cont: Some(StatusLineEncode::new(
                    transport,
                    version,
                    StatusCode::CONTINUE,
                )),
                body: None,
            }
        } else {
            Self {
                length,
                cont: None,
                body: Some(BodyDecode::new(transport, length)),
            }
        }
    }
}

impl<IO: AsyncRead + AsyncWrite + Unpin> AsyncRead for BodyDecodeWithContinue<IO> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();
        loop {
            if let Some(cont) = this.cont {
                match Pin::new(cont).poll(cx) {
                    Poll::Ready(Ok(transport)) => {
                        this.cont.take();
                        *this.body = Some(BodyDecode::new(transport, *this.length));
                    }
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                    Poll::Pending => return Poll::Pending,
                }
            }
            return Pin::new(this.body.as_mut().unwrap()).poll_read(cx, buf);
        }
    }
}

pub(crate) fn contains_continue(headers: &HeaderMap) -> bool {
    headers
        .get_all(EXPECT)
        .iter()
        .find(|v| v == &"100-continue")
        .is_some()
}
