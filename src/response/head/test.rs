use crate::response::head::ResponseHead;
use futures::executor::block_on;
use futures::io::Cursor;
use http::{StatusCode, Version};

const INPUT: &[u8] = b"HTTP/1.1 201 Created\r\nconnection: close\r\n\r\n";

async fn check(output: &ResponseHead<'_>) {
    assert_eq!(output.version(), Version::HTTP_11);
    assert_eq!(output.status(), StatusCode::CREATED);
    assert_eq!(
        output.headers().get("Connection").unwrap().as_bytes(),
        b"close"
    );
}

#[test]
fn test() {
    block_on(async {
        let head = ResponseHead::decode(Cursor::new(INPUT)).await.unwrap().1;
        check(&head).await;

        let mut transport = Cursor::new(Vec::new());
        head.encode(&mut transport).await.unwrap();
        assert_eq!(
            String::from_utf8(transport.into_inner()),
            String::from_utf8(INPUT.to_vec())
        );
    })
}
