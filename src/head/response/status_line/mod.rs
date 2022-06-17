mod encode;
pub use encode::*;

#[cfg(test)]
mod tests {
    use crate::StatusLineEncoder;
    use futures::executor::block_on;
    use futures::io::Cursor;
    use http::{StatusCode, Version};

    const INPUT: &[u8] = b"HTTP/1.1 100 Continue\r\n";

    #[test]
    fn test() {
        block_on(async {
            let mut transport = Cursor::new(Vec::new());
            StatusLineEncoder::default()
                .encode(&mut transport, Version::HTTP_11, StatusCode::CONTINUE)
                .await
                .unwrap();
            assert_eq!(
                String::from_utf8(transport.into_inner()),
                String::from_utf8(INPUT.to_vec())
            );
        })
    }
}
