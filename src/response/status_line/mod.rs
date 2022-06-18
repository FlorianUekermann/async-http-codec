use crate::buffer_write::{BufferWrite, BufferWriteState};
use crate::{status_line_encode, IoFutureState};
use futures::AsyncWrite;
use http::{StatusCode, Version};
use std::io;

#[derive(Clone, Debug)]
pub struct StatusLine {
    status: StatusCode,
    version: Version,
}

impl StatusLine {
    pub fn new(status: StatusCode, version: Version) -> Self {
        Self { status, version }
    }
    pub fn to_vec(&self) -> io::Result<Vec<u8>> {
        let mut buffer = Vec::with_capacity(1024);
        status_line_encode(&mut buffer, &self.version, &self.status)?;
        Ok(buffer)
    }
    pub fn encode<IO: AsyncWrite + Unpin>(&self, io: IO) -> BufferWrite<IO> {
        self.encode_state().into_future(io)
    }
    pub fn encode_state(&self) -> BufferWriteState {
        BufferWriteState::new(self.to_vec())
    }
    pub fn status(&self) -> StatusCode {
        self.status
    }
    pub fn version(&self) -> Version {
        self.version
    }
    pub fn status_mut(&mut self) -> &mut StatusCode {
        &mut self.status
    }
    pub fn version_mut(&mut self) -> &mut Version {
        &mut self.version
    }
}
