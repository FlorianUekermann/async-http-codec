use crate::{RequestHead, ResponseHead};
use http::header::HeaderName;
use http::{HeaderMap, HeaderValue, Method, Request, Response, StatusCode, Uri, Version};
use httparse::Header;
use std::error::Error;
use std::io;
use std::io::ErrorKind::InvalidData;

#[allow(deprecated)]
pub fn request_head_parse(buffer: &[u8], max_headers: usize) -> io::Result<RequestHead<'static>> {
    let mut headers = vec![httparse::EMPTY_HEADER; max_headers];
    let mut parsed_request = httparse::Request::new(&mut headers);
    if parsed_request
        .parse(buffer)
        .map_err(|err| io::Error::new(InvalidData, err.description()))?
        .is_partial()
    {
        return Err(io::Error::new(InvalidData, "malformed HTTP head"));
    }
    if parsed_request.version != Some(1) {
        return Err(io::Error::new(InvalidData, "unsupported HTTP version"));
    }
    let method = Method::from_bytes(parsed_request.method.unwrap_or("").as_bytes())
        .map_err(|err| io::Error::new(InvalidData, err.description()))?;
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
    copy_parsed_headers(headers, parsed_request.headers)?;
    Ok(RequestHead::from(request))
}

#[allow(deprecated)]
pub fn response_head_parse(buffer: &[u8], max_headers: usize) -> io::Result<ResponseHead<'static>> {
    let mut headers = vec![httparse::EMPTY_HEADER; max_headers];
    let mut parsed_response = httparse::Response::new(&mut headers);
    if parsed_response
        .parse(buffer)
        .map_err(|err| io::Error::new(InvalidData, err.description()))?
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
    Ok(ResponseHead::from(response))
}

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
