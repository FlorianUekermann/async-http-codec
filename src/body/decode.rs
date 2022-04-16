use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use futures::prelude::*;
use pin_project::pin_project;

#[pin_project]
pub struct BodyDecode<IO: AsyncRead + Unpin> {
    transport: IO,
    state: BodyDecodeState
}

impl<IO: AsyncRead + Unpin> BodyDecode<IO> {
    pub fn new(transport: IO, length: Option<u64>, chunked: bool) -> Self {
        BodyDecodeState::new(length, chunked).restore(transport)
    }
    pub fn checkpoint(self) -> (IO, BodyDecodeState) {
        (self.transport, self.state)
    }
}

impl<IO: AsyncRead + Unpin> AsyncRead for BodyDecode<IO> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        let this = self.project();
        this.state.poll_read(this.transport, cx, buf)
    }
}

pub struct BodyDecodeState {
    parser_state: Parser,
    _compression_state: (),
    remaining: u64,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Parser {
    MissingLength,
    FixedLength,
    Chunked(ChunkState),
    Failed,
    Done,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ChunkState {
    Size,
    SizeLF,
    Content,
    ContentCR,
    ContentLF,
    EndCR,
    EndLF,
}

fn err_kind<T>(kind: io::ErrorKind) -> Poll<io::Result<T>> {
    Poll::Ready(Err(kind.into()))
}

impl BodyDecodeState {
    pub fn new(length: Option<u64>, chunked: bool) -> Self {
        let (parser_state, remaining) = match (length, chunked) {
            (Some(length), false) => (Parser::FixedLength, length),
            (None, false) => (Parser::MissingLength, 0),
            (_, true) => (Parser::Chunked(ChunkState::Size), 0),
        };
        Self {
            parser_state,
            _compression_state: (),
            remaining,
        }
    }
    pub fn restore<IO: AsyncRead + Unpin>(self, transport: IO) -> BodyDecode<IO> {
        BodyDecode {
            transport,
            state: self,
        }
    }
    fn poll_read<IO: AsyncRead + Unpin>(&mut self, mut transport: IO, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        loop {
            let max_read_size = match self.parser_state {
                Parser::Failed => return err_kind(io::ErrorKind::BrokenPipe),
                Parser::Done => return Poll::Ready(Ok(0)),
                Parser::MissingLength => buf.len(),
                Parser::FixedLength | Parser::Chunked(ChunkState::Content) => {
                    if buf.len() as u64 > self.remaining {
                        self.remaining as usize
                    } else {
                        buf.len()
                    }
                }
                Parser::Chunked(chunked_state) => {
                    let mut next = [0u8];
                    match Pin::new(&mut transport).poll_read(cx, &mut next) {
                        Poll::Ready(Err(err)) => {
                            self.parser_state = Parser::Failed;
                            return Poll::Ready(Err(err));
                        }
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Ok(0)) => {
                            self.parser_state = Parser::Failed;
                            return err_kind(io::ErrorKind::UnexpectedEof);
                        }
                        Poll::Ready(Ok(_)) => {
                            self.parser_state = match (chunked_state, next[0]) {
                                (ChunkState::Size, b'\r') => Parser::Chunked(ChunkState::SizeLF),
                                (ChunkState::Size, hex_digit) => {
                                    self.remaining *= 16;
                                    self.remaining += match hex_digit {
                                        b'0'..=b'9' => 0 + hex_digit - b'0',
                                        b'a'..=b'f' => 10 + hex_digit - b'a',
                                        b'A'..=b'F' => 10 + hex_digit - b'A',
                                        _ => {
                                            self.parser_state = Parser::Failed;
                                            return err_kind(io::ErrorKind::InvalidData);
                                        }
                                    } as u64;
                                    Parser::Chunked(ChunkState::Size)
                                },
                                (ChunkState::SizeLF, b'\n') => match self.remaining{
                                    0 => Parser::Chunked(ChunkState::EndCR),
                                    _ => Parser::Chunked(ChunkState::Content)
                                },
                                (ChunkState::Content, _) => unreachable!(),
                                (ChunkState::ContentCR, b'\r') => Parser::Chunked(ChunkState::ContentLF),
                                (ChunkState::ContentLF, b'\n') => Parser::Chunked(ChunkState::Size),
                                (ChunkState::EndCR, b'\r') => Parser::Chunked(ChunkState::EndLF),
                                (ChunkState::EndLF, b'\n') => Parser::Done,
                                (_, _) => return err_kind(io::ErrorKind::InvalidData)
                            }
                        }
                    }
                    continue;
                }
            };
            return match Pin::new(&mut transport).poll_read(cx, &mut buf[0..max_read_size]) {
                Poll::Ready(Err(err)) => {
                    self.parser_state = Parser::Failed;
                    Poll::Ready(Err(err))
                }
                Poll::Ready(Ok(0)) => {
                    if self.parser_state == Parser::MissingLength {
                        self.parser_state = Parser::Done;
                        return Poll::Ready(Ok(0));
                    }
                    self.parser_state = Parser::Failed;
                    err_kind(io::ErrorKind::UnexpectedEof)
                }
                Poll::Ready(Ok(n)) => match self.parser_state {
                    Parser::MissingLength => Poll::Ready(Ok(n)),
                    _ => {
                        self.remaining -= n as u64;
                        if self.remaining == 0 {
                            self.parser_state = match self.parser_state {
                                Parser::FixedLength => Parser::Done,
                                Parser::Chunked(ChunkState::Content) => Parser::Chunked(ChunkState::ContentCR),
                                _ => unreachable!(),
                            }
                        }
                        Poll::Ready(Ok(n))
                    }
                },
                Poll::Pending => Poll::Pending,
            }
        }
    }
}
