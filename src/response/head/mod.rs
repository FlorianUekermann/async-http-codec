use crate::buffer_write::{BufferWrite, BufferWriteState};
use crate::response::status_line::StatusLine;
use crate::{header_encode, IoFutureState};
use futures::executor::block_on;
use futures::io::Cursor;
use futures::AsyncWrite;
use http::response::Parts;
use http::{HeaderMap, Response, StatusCode, Version};
use std::borrow::Cow;
use std::io;

#[derive(Clone, Debug)]
pub struct ResponseHead<'a> {
    status_line: StatusLine,
    headers: Cow<'a, HeaderMap>,
}

impl<'a> ResponseHead<'a> {
    pub fn ref_parts(parts: &'a Parts) -> Self {
        Self {
            status_line: StatusLine::new(parts.status, parts.version),
            headers: Cow::Borrowed(&parts.headers),
        }
    }
    pub fn ref_response<B>(response: &'a Response<B>) -> Self {
        Self {
            status_line: StatusLine::new(response.status(), response.version()),
            headers: Cow::Borrowed(&response.headers()),
        }
    }
    pub fn to_owned(self) -> ResponseHead<'static> {
        ResponseHead {
            status_line: self.status_line,
            headers: Cow::Owned(self.headers.into_owned()),
        }
    }
    pub fn to_vec(&self) -> io::Result<Vec<u8>> {
        let mut buffer = Vec::with_capacity(8192);
        block_on(self.status_line.encode(Cursor::new(&mut buffer)))?;
        header_encode(&mut buffer, &self.headers)?;
        Ok(buffer)
    }
    pub fn encode<IO: AsyncWrite + Unpin>(&self, io: IO) -> BufferWrite<IO> {
        self.encode_state().into_future(io)
    }
    pub fn encode_state(&self) -> BufferWriteState {
        BufferWriteState::new(self.to_vec())
    }
    pub fn status(&self) -> StatusCode {
        self.status_line.status()
    }
    pub fn version(&self) -> Version {
        self.status_line.version()
    }
    pub fn headers(&self) -> &HeaderMap {
        self.headers.as_ref()
    }
    pub fn status_mut(&mut self) -> &mut StatusCode {
        self.status_line.status_mut()
    }
    pub fn version_mut(&mut self) -> &mut Version {
        self.status_line.version_mut()
    }
    pub fn headers_mut(&mut self) -> &mut HeaderMap {
        self.headers.to_mut()
    }
}

impl From<Parts> for ResponseHead<'static> {
    fn from(parts: Parts) -> Self {
        Self {
            status_line: StatusLine::new(parts.status, parts.version),
            headers: Cow::Owned(parts.headers),
        }
    }
}

impl<'a> From<ResponseHead<'a>> for Parts {
    fn from(head: ResponseHead<'a>) -> Self {
        let mut parts = Response::new(()).into_parts().0;
        parts.status = head.status_line.status();
        parts.version = head.status_line.version();
        parts.headers = head.headers.into_owned();
        parts
    }
}

impl<B> From<Response<B>> for ResponseHead<'static> {
    fn from(response: Response<B>) -> Self {
        response.into_parts().0.into()
    }
}

impl<'a> From<ResponseHead<'a>> for Response<()> {
    fn from(head: ResponseHead<'a>) -> Self {
        let parts: Parts = head.into();
        Response::from_parts(parts, ())
    }
}
