use crate::RequestHead;
use http::header::HeaderName;
use http::{HeaderValue, Method, Request, Uri, Version};
use std::io;
use std::io::ErrorKind::InvalidData;
use std::io::Read;

enum TerminatorOverlapInBuffer {
    None,
    Partial(usize),
    Complete,
}

pub struct RequestHeadParse {
    buffer: Vec<u8>,
    terminator: TerminatorOverlapInBuffer,
    max_headers: usize,
}

impl<'a> RequestHeadParse {
    const END: &'a [u8] = b"\r\n\r\n";
    pub fn new(max_buffer: usize, max_headers: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(max_buffer),
            terminator: TerminatorOverlapInBuffer::None,
            max_headers,
        }
    }
    pub fn read_data<T: Read>(&mut self, rd: &mut T) -> Result<usize, std::io::Error> {
        let mut chunks = [0u8; Self::END.len()];
        loop {
            match self.terminator {
                TerminatorOverlapInBuffer::None => {
                    rd.read_exact(&mut chunks)?;
                    self.buffer.extend_from_slice(&chunks);
                    if chunks == Self::END {
                        self.terminator = TerminatorOverlapInBuffer::Complete;
                    } else {
                        for i in 1..Self::END.len() {
                            let window_size = Self::END.len() - i;
                            if chunks[i..] == Self::END[0..window_size] {
                                self.terminator = TerminatorOverlapInBuffer::Partial(i);
                            }
                        }
                    }
                }
                TerminatorOverlapInBuffer::Complete => {
                    break;
                }
                TerminatorOverlapInBuffer::Partial(x) => {
                    let chunks = &mut chunks[0..x];
                    rd.read_exact(chunks)?;
                    self.buffer.extend_from_slice(chunks);
                    match chunks[0..x] == Self::END[(Self::END.len() - x)..] {
                        true => self.terminator = TerminatorOverlapInBuffer::Complete,
                        false => self.terminator = TerminatorOverlapInBuffer::None,
                    }
                }
            }
        }
        Ok(self.buffer.len())
    }
    pub fn try_take_head(&mut self) -> io::Result<RequestHead> {
        let mut headers = vec![httparse::EMPTY_HEADER; self.max_headers];
        let mut parsed_request = httparse::Request::new(&mut headers);
        if parsed_request
            .parse(self.buffer.as_ref())
            .map_err(|err| io::Error::new(InvalidData, err.to_string()))?
            .is_partial()
        {
            return Err(io::Error::new(InvalidData, "malformed HTTP head"));
        }
        if parsed_request.version != Some(1) {
            return Err(io::Error::new(InvalidData, "unsupported HTTP version"));
        }
        let method = Method::from_bytes(parsed_request.method.unwrap_or("").as_bytes())
            .map_err(|err| io::Error::new(InvalidData, err.to_string()))?;
        let uri = parsed_request
            .path
            .unwrap_or("")
            .parse::<Uri>()
            .map_err(|_| io::Error::new(InvalidData, "invalid uri"))?;
        let mut request = Request::new(());
        *request.method_mut() = method;
        *request.uri_mut() = uri;
        *request.version_mut() = Version::HTTP_11;
        let headers = request.headers_mut();
        headers.reserve(parsed_request.headers.len());
        for header in parsed_request.headers {
            headers.append(
                HeaderName::from_bytes(header.name.as_bytes())
                    .map_err(|_| io::Error::new(InvalidData, "invalid header name"))?,
                HeaderValue::from_bytes(header.value)
                    .map_err(|_| io::Error::new(InvalidData, "invalid header value"))?,
            );
        }
        Ok(RequestHead::from(request))
    }
}
