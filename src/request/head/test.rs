use std::borrow::Cow;

use crate::request::head::RequestHead;
use futures::executor::block_on;
use futures::io::Cursor;
use http::{Method, Version, Uri, HeaderMap};

use super::parse::RequestHeadParse;

const INPUT: &[u8] = b"GET / HTTP/1.1\r\nhost: www.example.com\r\nconnection: close\r\n\r\n";

async fn check(head: &RequestHead<'_>) {
    assert_eq!(head.version(), Version::HTTP_11);
    assert_eq!(head.method(), Method::GET);
    assert_eq!(head.uri(), "/");
    assert_eq!(
        head.headers.get("host").unwrap().as_bytes(),
        b"www.example.com"
    );
    assert_eq!(head.headers.get("connection").unwrap().as_bytes(), b"close");
}

#[test]
fn test() {
    block_on(async {
        let head = RequestHead::decode(Cursor::new(INPUT)).await.unwrap().1;
        check(&head).await;

        let mut transport = Cursor::new(Vec::new());
        head.encode(&mut transport).await.unwrap();
        assert_eq!(
            String::from_utf8(transport.into_inner()),
            String::from_utf8(INPUT.to_vec())
        );
    })
}

#[test]
fn test_request_head_parse() {
    let uri = Uri::builder().scheme("https").authority("google.com").path_and_query("/").build().unwrap();
    
    let header_map = HeaderMap::new();
    
    let req_head = RequestHead::new(Method::GET, Cow::Borrowed(&uri), Version::HTTP_11, Cow::Borrowed(&header_map));

    let mut req_head_parse = RequestHeadParse::new(8096,header_map.len());

    let mut cursor = std::io::Cursor::new(req_head.to_vec().unwrap());
    
    let _ = req_head_parse.read_data(&mut cursor);
    let req_header_recv = req_head_parse.try_take_head().unwrap();

    assert_eq!(req_head.method, req_header_recv.method);
    assert_eq!(req_head.uri, req_header_recv.uri);
    assert_eq!(req_head.version, req_header_recv.version);
    assert_eq!(req_head.headers, req_header_recv.headers);

}
