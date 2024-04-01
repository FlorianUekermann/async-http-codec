use std::io::{self, ErrorKind::InvalidData, Read};

use http::{response::Parts, Response, StatusCode, Version};

use crate::internal::{dec_helpers::copy_parsed_headers, terminator::TerminatorOverlap};

pub struct ResponseHeadParse<'a> {
    buffer: Vec<u8>,
    terminator: TerminatorOverlap<'a>,
    max_headers: usize,
}

impl<'a> ResponseHeadParse<'a> {
    const END: &'a [u8] = b"\r\n\r\n";
    pub fn new(max_buffer: usize, max_headers: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(max_buffer),
            terminator: TerminatorOverlap::new(Self::END),
            max_headers,
        }
    }
    pub fn read_data<T: Read>(&mut self, rd: &mut T) -> Result<usize, std::io::Error> {
        let mut chunks = [0u8; Self::END.len()];
        while !self.terminator.done() {
            let chunks = self.terminator.max_read_buf(&mut chunks);
            if self.buffer.capacity() - self.buffer.len() < chunks.len() {
                return Err(std::io::ErrorKind::OutOfMemory.into());
            }
            rd.read_exact(chunks)?;
            self.terminator.process(&chunks);
            self.buffer.extend_from_slice(chunks);
        }
        Ok(self.buffer.len())
    }
    pub fn try_take_head(&mut self) -> io::Result<Parts> {
        let mut headers = vec![httparse::EMPTY_HEADER; self.max_headers];
        let mut parsed_response = httparse::Response::new(&mut headers);
        if parsed_response
            .parse(self.buffer.as_slice())
            .map_err(|err| io::Error::new(InvalidData, err.to_string()))?
            .is_partial()
        {
            return Err(io::Error::new(InvalidData, "malformed HTTP head"));
        }
        if parsed_response.version != Some(1) {
            return Err(io::Error::new(InvalidData, "unsupported HTTP version"));
        }
        let mut response = Response::new(());
        *response.version_mut() = Version::HTTP_11;
        *response.status_mut() = StatusCode::from_u16(parsed_response.code.unwrap())
            .map_err(|_| io::Error::new(InvalidData, "invalid status code"))?;
        let headers = response.headers_mut();
        copy_parsed_headers(headers, parsed_response.headers)?;
        Ok(response.into_parts().0)
    }
}
