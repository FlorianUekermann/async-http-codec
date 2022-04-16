mod encode;
mod decode;

pub use encode::*;
pub use decode::*;

#[cfg(test)]
mod tests {
    use futures::executor::block_on;
    use futures::io::Cursor;
    use futures::prelude::*;
    use crate::body::{BodyDecode};

    const CHUNKED: &[u8] = b"\
6\r\n\
hello \r\n\
13\r\n\
world! hello world!\r\n\
0\r\n\
\r\n";
    const DECODED: &[u8] = b"hello world! hello world!";

    #[test]
    fn chunked() {
        block_on(
            async {
                let mut decoded = Vec::new();
                BodyDecode::new(Cursor::new(CHUNKED), None, true)
                    .read_to_end(&mut decoded).await.unwrap();
                assert_eq!(
                    String::from_utf8(decoded).unwrap(),
                    String::from_utf8(DECODED.to_vec()).unwrap()
                );
            })
    }

    #[test]
    fn fixed_length() {
        block_on(
            async {
                const LENGTH: u64 = 10;
                let mut decoded = Vec::new();
                BodyDecode::new(Cursor::new(DECODED), Some(LENGTH), false)
                    .read_to_end(&mut decoded).await.unwrap();
                assert_eq!(
                    String::from_utf8(decoded).unwrap(),
                    String::from_utf8(DECODED[0..LENGTH as usize].to_vec()).unwrap()
                );
            })
    }

    #[test]
    fn missing_length() {
        block_on(
            async {
                let mut decoded = Vec::new();
                BodyDecode::new(Cursor::new(DECODED), None, false)
                    .read_to_end(&mut decoded).await.unwrap();
                assert_eq!(
                    String::from_utf8(decoded).unwrap(),
                    String::from_utf8(DECODED.to_vec()).unwrap()
                );
            })
    }
}
