use anyhow::bail;
use http::HeaderMap;
use std::io::Write;

pub(crate) fn header_encode(buffer: &mut Vec<u8>, headers: &HeaderMap) -> anyhow::Result<()> {
    for (k, v) in headers {
        let v = match v.to_str() {
            Err(_) => bail!("invalid character in header value"),
            Ok(v) => v,
        };
        writeln!(buffer, "{}: {}\r", k, v)?;
    }
    writeln!(buffer, "\r")?;
    Ok(())
}
