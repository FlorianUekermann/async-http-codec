mod encode;

use crate::buffer_write::{BufferWrite, BufferWriteState};
use crate::{header_encode, IoFutureState};
use futures::AsyncWrite;
use http::request::Parts;
use http::{HeaderMap, Method, Request, Uri, Version};
use std::borrow::Cow;
use std::io;

#[derive(Clone, Debug)]
pub struct RequestHead<'a> {
    method: Method,
    uri: Cow<'a, Uri>,
    version: Version,
    headers: Cow<'a, HeaderMap>,
}

impl<'a> RequestHead<'a> {
    pub fn ref_parts(parts: &'a Parts) -> Self {
        Self {
            method: parts.method.clone(),
            uri: Cow::Borrowed(&parts.uri),
            version: parts.version,
            headers: Cow::Borrowed(&parts.headers),
        }
    }
    pub fn ref_request<B>(request: &'a Request<B>) -> Self {
        Self {
            method: request.method().clone(),
            uri: Cow::Borrowed(&request.uri()),
            version: request.version(),
            headers: Cow::Borrowed(&request.headers()),
        }
    }
    pub fn to_owned(self) -> RequestHead<'static> {
        RequestHead {
            method: self.method,
            uri: Cow::Owned(self.uri.into_owned()),
            version: self.version,
            headers: Cow::Owned(self.headers.into_owned()),
        }
    }
    pub fn to_vec(&self) -> io::Result<Vec<u8>> {
        use std::io::Write;
        let mut buffer = Vec::with_capacity(8192);
        writeln!(buffer, "{} {} {:?}\r", self.method, &self.uri, self.version)?;
        header_encode(&mut buffer, &self.headers)?;
        Ok(buffer)
    }
    pub fn encode<IO: AsyncWrite + Unpin>(&self, io: IO) -> BufferWrite<IO> {
        self.encode_state().into_future(io)
    }
    pub fn encode_state(&self) -> BufferWriteState {
        BufferWriteState::new(self.to_vec())
    }
}

impl From<Parts> for RequestHead<'static> {
    fn from(parts: Parts) -> Self {
        Self {
            method: parts.method.clone(),
            uri: Cow::Owned(parts.uri),
            version: parts.version,
            headers: Cow::Owned(parts.headers),
        }
    }
}

impl<'a> From<RequestHead<'a>> for Parts {
    fn from(head: RequestHead<'a>) -> Self {
        let mut parts = Request::new(()).into_parts().0;
        parts.method = head.method;
        parts.uri = head.uri.into_owned();
        parts.version = head.version;
        parts.headers = head.headers.into_owned();
        parts
    }
}

impl<B> From<Request<B>> for RequestHead<'static> {
    fn from(request: Request<B>) -> Self {
        request.into_parts().0.into()
    }
}

impl<'a> From<RequestHead<'a>> for Request<()> {
    fn from(head: RequestHead<'a>) -> Self {
        let parts: Parts = head.into();
        Request::from_parts(parts, ())
    }
}
