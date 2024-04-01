use crate::request::head::RequestHead;
use futures::executor::block_on;
use futures::io::Cursor;
use http::{Method, Version};

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
    let mut parser = RequestHeadParse::new(8096, 10);
    let mut input = INPUT;
    let size = parser.read_data(&mut input).unwrap();
    println!("{}", size);
    let head = parser.try_take_head().unwrap();
    block_on( async {check(&head).await });
}
