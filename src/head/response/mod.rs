mod decode;
mod encode;

pub use decode::*;
pub use encode::*;

#[cfg(test)]
mod tests {
    use futures::executor::block_on;
    use futures::io::Cursor;
    use crate::ResponseHeadDecoder;
    use crate::ResponseHeadEncoder;
    use http::response::Parts;
    use http::{StatusCode, Version};

    const INPUT: &[u8] = b"HTTP/1.1 200 OK\r\nconnection: close\r\n\r\n";

    async fn check(output: &Parts) {
        assert_eq!(output.version, Version::HTTP_11);
        assert_eq!(output.status, StatusCode::OK);
        assert_eq!(
            output.headers.get("Connection").unwrap().as_bytes(),
            b"close"
        );
    }

    #[test]
    fn test() {
        block_on(async {
            let head = ResponseHeadDecoder::default()
                .decode(Cursor::new(INPUT))
                .await
                .unwrap()
                .1;
            check(&head).await;

            let mut transport = Cursor::new(Vec::new());
            ResponseHeadEncoder::default()
                .encode(&mut transport, head)
                .await
                .unwrap();
            assert_eq!(
                String::from_utf8(transport.into_inner()),
                String::from_utf8(INPUT.to_vec())
            );
        })
    }
}
