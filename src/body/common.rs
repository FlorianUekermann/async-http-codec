use anyhow::bail;
use http::header::{CONTENT_LENGTH, TRANSFER_ENCODING};
use http::HeaderMap;

pub(crate) fn length_from_headers(headers: &HeaderMap) -> anyhow::Result<Option<u64>> {
    let mut chunked = false;
    for v in headers.get_all(TRANSFER_ENCODING) {
        if v != "chunked" {
            bail!("unsupported Transfer-Encoding: {:?}", v)
        }
        chunked = true;
    }
    if chunked {
        Ok(None)
    } else if let Some(v) = headers.get(CONTENT_LENGTH) {
        Ok(Some(v.to_str()?.parse()?))
    } else {
        Ok(Some(0))
    }
}
