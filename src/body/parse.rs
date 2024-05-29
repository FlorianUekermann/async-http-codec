use std::io::Read;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetaInfoKind {
    ContentLength,
    HeaderCRLF,
    ContentCRLF,
    FinalCRLF,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseState {
    ReadMetaInfo(MetaInfoKind),
    CopyContent,
    Done,
}

#[derive(Debug)]
pub struct BodyParseChunked {
    overlap: usize,
    n: usize,
    state: ParseState,
}

impl BodyParseChunked {
    pub fn new() -> Self {
        BodyParseChunked {
            overlap: 0,
            n: 0,
            state: ParseState::ReadMetaInfo(MetaInfoKind::ContentLength),
        }
    }
    pub fn process_data<T: Read>(
        &mut self,
        rd: &mut T,
        out: &mut [u8],
    ) -> Result<usize, std::io::Error> {
        let mut read_until_term = [0u8; 4];
        let mut bytes_written_to_out = 0;
        loop {
            match self.state {
                ParseState::ReadMetaInfo(_) => {
                    let n_max = self.guaranteed_read();
                    let n_read = rd.read(&mut read_until_term[0..n_max])?;
                    let mut iter = read_until_term[0..n_read].into_iter();

                    while let Some(&byte) = iter.next() {
                        match (byte as char, self.state) {
                            (
                                '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' | '0' | 'A'
                                | 'B' | 'C' | 'D' | 'E' | 'F',
                                ParseState::ReadMetaInfo(MetaInfoKind::ContentLength),
                            ) if self.overlap == 0 => {
                                self.n =
                                    self.n * 16 + (byte as char).to_digit(16).unwrap() as usize;
                                if self.n == 0 {
                                    self.state = ParseState::ReadMetaInfo(MetaInfoKind::HeaderCRLF);
                                }
                            }
                            (
                                '\r',
                                ParseState::ReadMetaInfo(
                                    MetaInfoKind::HeaderCRLF
                                    | MetaInfoKind::FinalCRLF
                                    | MetaInfoKind::ContentLength
                                    | MetaInfoKind::ContentCRLF,
                                ),
                            ) if self.overlap == 0 => {
                                if ParseState::ReadMetaInfo(MetaInfoKind::ContentLength)
                                    == self.state
                                {
                                    self.state = ParseState::ReadMetaInfo(MetaInfoKind::HeaderCRLF)
                                }
                                self.overlap = 1;
                            }
                            ('\n', ParseState::ReadMetaInfo(MetaInfoKind::HeaderCRLF))
                                if self.overlap == 1 =>
                            {
                                self.overlap = 0;
                                if self.n == 0 {
                                    self.state = ParseState::ReadMetaInfo(MetaInfoKind::FinalCRLF);
                                } else {
                                    self.state = ParseState::CopyContent;
                                }
                            }
                            ('\n', ParseState::ReadMetaInfo(MetaInfoKind::ContentCRLF))
                                if self.overlap == 1 =>
                            {
                                self.overlap = 0;
                                self.state = ParseState::ReadMetaInfo(MetaInfoKind::ContentLength);
                            }
                            ('\n', ParseState::ReadMetaInfo(MetaInfoKind::FinalCRLF))
                                if self.overlap == 1 =>
                            {
                                self.overlap = 0;
                                self.state = ParseState::Done;
                            }
                            _ => {
                                return Err(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    format!("Unexpected character while reading CRLF, last byte {}, self: {:?}", byte, self),
				));
                            }
                        }
                    }
                }
                ParseState::CopyContent => {
                    let next_bytes_to_out =
                        self.guaranteed_read().min(out.len() - bytes_written_to_out);
                    let bytes_written = rd.read(
                        &mut out[bytes_written_to_out
                            ..(bytes_written_to_out + next_bytes_to_out as usize)],
                    )?;
                    self.n -= bytes_written;
                    bytes_written_to_out += bytes_written;

                    if bytes_written_to_out >= out.len() {
                        if bytes_written_to_out > out.len() {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "Unexpected character in Content-Length Header",
                            ));
                        }
                        return Ok(bytes_written_to_out);
                    }

                    if self.n == 0 {
                        self.state = ParseState::ReadMetaInfo(MetaInfoKind::ContentCRLF)
                    }
                }
                ParseState::Done => return Ok(bytes_written_to_out),
            }
        }
    }
    pub fn is_finished(&self) -> bool {
        self.state == ParseState::Done
    }
    /// returns min number of unprocessed bytes (0 if done)
    pub fn guaranteed_read(&self) -> usize {
        match self {
            BodyParseChunked {
                overlap: 0,
                n: 0,
                state: ParseState::ReadMetaInfo(MetaInfoKind::HeaderCRLF),
            } => 4,
            BodyParseChunked {
                overlap: 0,
                n: 0,
                state: ParseState::ReadMetaInfo(MetaInfoKind::ContentLength),
            } => 3,
            BodyParseChunked {
                overlap: _,
                n: _,
                state: ParseState::ReadMetaInfo(_),
            } => 2 - self.overlap,
            BodyParseChunked {
                overlap: 0,
                n: _,
                state: ParseState::CopyContent,
            } => self.n,
            _ => 1,
        }
    }
}
