#[cfg(test)]
mod test;

use crate::internal::buffer_decode::{BufferDecode, BufferDecodeState};
use crate::internal::buffer_write::{BufferWrite, BufferWriteState};
use crate::internal::dec_helpers::response_head_parse;
use crate::internal::enc_helpers::{header_encode, status_line_encode};
use crate::internal::io_future::{IoFutureState, IoFutureWithOutputState};
use futures::{AsyncRead, AsyncWrite};
use http::response::Parts;
use http::{HeaderMap, Response, StatusCode, Version};
use std::borrow::Cow;
use std::io;

#[derive(Clone, Debug)]
pub struct ResponseHead<'a> {
    status: StatusCode,
    version: Version,
    headers: Cow<'a, HeaderMap>,
}

impl<'a> ResponseHead<'a> {
    pub fn new(status: StatusCode, version: Version, headers: Cow<'a, HeaderMap>) -> Self {
        Self {
            status,
            version,
            headers: headers.into(),
        }
    }
    pub fn ref_parts(parts: &'a Parts) -> Self {
        Self {
            status: parts.status,
            version: parts.version,
            headers: Cow::Borrowed(&parts.headers),
        }
    }
    pub fn ref_response<B>(response: &'a Response<B>) -> Self {
        Self {
            status: response.status(),
            version: response.version(),
            headers: Cow::Borrowed(&response.headers()),
        }
    }
    pub fn to_owned(self) -> ResponseHead<'static> {
        ResponseHead {
            status: self.status,
            version: self.version,
            headers: Cow::Owned(self.headers.into_owned()),
        }
    }
    pub fn to_vec(&self) -> io::Result<Vec<u8>> {
        let mut buffer = Vec::with_capacity(8192);
        status_line_encode(&mut buffer, &self.version, &self.status)?;
        header_encode(&mut buffer, &self.headers)?;
        Ok(buffer)
    }
    pub fn encode<IO: AsyncWrite + Unpin>(&self, io: IO) -> BufferWrite<IO> {
        self.encode_state().into_future(io)
    }
    pub fn encode_state(&self) -> BufferWriteState {
        BufferWriteState::new(self.to_vec())
    }
    pub fn decode<IO: AsyncRead + Unpin>(io: IO) -> BufferDecode<IO, Self> {
        Self::decode_state().into_future(io)
    }
    pub fn decode_state() -> BufferDecodeState<Self> {
        BufferDecodeState::new(8192, 128, &response_head_parse)
    }
    pub fn status(&self) -> StatusCode {
        self.status
    }
    pub fn version(&self) -> Version {
        self.version
    }
    pub fn headers(&self) -> &HeaderMap {
        self.headers.as_ref()
    }
    pub fn status_mut(&mut self) -> &mut StatusCode {
        &mut self.status
    }
    pub fn version_mut(&mut self) -> &mut Version {
        &mut self.version
    }
    pub fn headers_mut(&mut self) -> &mut HeaderMap {
        self.headers.to_mut()
    }
}

impl From<Parts> for ResponseHead<'static> {
    fn from(parts: Parts) -> Self {
        Self {
            status: parts.status,
            version: parts.version,
            headers: Cow::Owned(parts.headers),
        }
    }
}

impl<'a> From<ResponseHead<'a>> for Parts {
    fn from(head: ResponseHead<'a>) -> Self {
        let mut parts = Response::new(()).into_parts().0;
        parts.status = head.status;
        parts.version = head.version;
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
