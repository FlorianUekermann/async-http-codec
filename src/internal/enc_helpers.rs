use http::{HeaderMap, StatusCode, Version};
use std::io;
use std::io::ErrorKind::InvalidData;
use std::io::Write;

pub(crate) fn header_encode(buffer: &mut Vec<u8>, headers: &HeaderMap) -> io::Result<()> {
    for (k, v) in headers {
        let v = v
            .to_str()
            .map_err(|_| io::Error::new(InvalidData, "invalid character in header value"))?;
        writeln!(buffer, "{}: {}\r", k, v)?;
    }
    writeln!(buffer, "\r")?;
    Ok(())
}

pub(crate) fn status_line_encode(
    buffer: &mut Vec<u8>,
    version: &Version,
    status: &StatusCode,
) -> io::Result<()> {
    writeln!(buffer, "{:?} {}\r", version, status)?;
    Ok(())
}
