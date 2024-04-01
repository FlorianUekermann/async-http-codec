use http::{HeaderMap, HeaderName, HeaderValue};
use httparse::Header;
use std::io::{self, ErrorKind::InvalidData};

pub(crate) fn copy_parsed_headers(trg: &mut HeaderMap, parsed: &[Header]) -> io::Result<()> {
    trg.reserve(parsed.len());
    for header in parsed {
        trg.append(
            HeaderName::from_bytes(header.name.as_bytes())
                .map_err(|_| io::Error::new(InvalidData, "invalid header name"))?,
            HeaderValue::from_bytes(header.value)
                .map_err(|_| io::Error::new(InvalidData, "invalid header value"))?,
        );
    }
    Ok(())
}
