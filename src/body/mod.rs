pub(crate) mod common;
mod decode;
mod encode;
mod parse;

pub use decode::*;
pub use encode::*;
pub use parse::*;

#[cfg(test)]
mod tests {
    use crate::BodyDecode;
    use crate::BodyEncode;
    use crate::BodyParseChunked;
    use futures::executor::block_on;
    use futures::io::Cursor;
    use futures::prelude::*;
    use rand::{thread_rng, Rng};
    use std::io::SeekFrom;

    const CHUNKED: &[u8] = b"\
6\r\n\
hello \r\n\
13\r\n\
world! hello world!\r\n\
0\r\n\
\r\n";
    const CHUNKED_ONE_LINE: &[u8] = b"\
6\r\n\
hello \r\n\
0\r\n\
\r\n";
    const CHUNKED_TWO_LINES: &[u8] = b"\
6\r\n\
hello \r\n\
13\r\n\
world! hello world!\r\n\
0\r\n\
\r\n";
    const CHUNKED_THREE_LINES: &[u8] = b"\
6\r\n\
hello \r\n\
13\r\n\
world! hello world!\r\n\
17\r\n\
world! hello world!bla.\r\n\
0\r\n\
\r\n";
    const CHUNKED_ARRAY: [&[u8]; 3] = [CHUNKED_ONE_LINE, CHUNKED_TWO_LINES, CHUNKED_THREE_LINES];
    const DECODED_ONE_LINE: &str = "hello ";
    const DECODED_TWO_LINES: &str = "hello world! hello world!";
    const DECODED_THREE_LINES: &str = "hello world! hello world!world! hello world!bla.";
    const DECODED_ARRAY: [&str; 3] = [DECODED_ONE_LINE, DECODED_TWO_LINES, DECODED_THREE_LINES];
    const DECODED: &[u8] = b"hello world! hello world!";
    const CHUNKED_PARTS: [&[u8]; 2] = [b"hello ", b"world! hello world!"];

    fn body_parse_chunked_decode_length<const READ_LENGTH: usize, const OUT_LENGTH: usize>(
        array_index: usize,
    ) {
        let mut parser = BodyParseChunked::new();
        let mut out = [0; OUT_LENGTH];
        let mut bytes_read = 0;
        let mut content: &[u8] = &CHUNKED_ARRAY[array_index][..];

        while !parser.is_finished() {
            let next_bytes = parser.process_data(&mut content, &mut out).unwrap();
            assert_eq!(
                &out[0..next_bytes],
                DECODED_ARRAY[array_index][bytes_read..bytes_read + next_bytes].as_bytes()
            );
            bytes_read += next_bytes;
        }
    }
    #[test]
    fn body_parse_chunked_decode() {
        for i in 0..3 {
            body_parse_chunked_decode_length::<3, 5>(i);
            body_parse_chunked_decode_length::<3, 10>(i);
            body_parse_chunked_decode_length::<100, 5>(i);
            body_parse_chunked_decode_length::<100, 100>(i);
        }
    }
    #[test]
    fn decode_chunked() {
        block_on(async {
            let mut decoded = Vec::new();
            BodyDecode::new(Cursor::new(CHUNKED), None)
                .read_to_end(&mut decoded)
                .await
                .unwrap();
            assert_eq!(
                String::from_utf8(decoded).unwrap(),
                String::from_utf8(DECODED.to_vec()).unwrap()
            );
        })
    }

    #[test]
    fn decode_fixed_length() {
        block_on(async {
            const LENGTH: u64 = 10;
            let mut decoded = Vec::new();
            BodyDecode::new(Cursor::new(DECODED), Some(LENGTH))
                .read_to_end(&mut decoded)
                .await
                .unwrap();
            assert_eq!(
                String::from_utf8(decoded).unwrap(),
                String::from_utf8(DECODED[0..LENGTH as usize].to_vec()).unwrap()
            );
        })
    }

    #[test]
    fn decode_empty() {
        block_on(async {
            let mut decoded = Vec::new();
            BodyDecode::new(Cursor::new(""), Some(0))
                .read_to_end(&mut decoded)
                .await
                .unwrap();
            assert_eq!(
                String::from_utf8(decoded).unwrap(),
                String::from_utf8(b"".to_vec()).unwrap()
            );
        })
    }

    #[test]
    fn encode_chunked_flush() {
        block_on(async {
            let mut transport = Cursor::new(Vec::new());
            let mut encode = BodyEncode::new(&mut transport, None);
            for part in CHUNKED_PARTS {
                encode.write(part).await.unwrap();
                encode.flush().await.unwrap();
            }
            encode.close().await.unwrap();
            assert_eq!(
                String::from_utf8(transport.into_inner()).unwrap(),
                String::from_utf8(CHUNKED.to_vec()).unwrap()
            );
        })
    }

    #[test]
    fn encode_chunked_long() {
        block_on(async {
            let mut input = [0u8; 1500];
            thread_rng().fill(&mut input[..]);

            let mut transport = Cursor::new(Vec::new());
            let mut encode = BodyEncode::new(&mut transport, None);
            encode.write_all(&input).await.unwrap();
            encode.close().await.unwrap();

            let mut output = Vec::new();
            transport.seek(SeekFrom::Start(0)).await.unwrap();
            BodyDecode::new(transport, None)
                .read_to_end(&mut output)
                .await
                .unwrap();
            assert_eq!(&input, &output[..]);
        })
    }
}
