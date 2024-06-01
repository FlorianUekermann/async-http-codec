use std::io::Read;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminatorRead {
    NoneRead,
    CRRead,
}

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
    terminator: TerminatorRead,
    n: usize,
    state: ParseState,
}

impl BodyParseChunked {
    pub fn new() -> Self {
        BodyParseChunked {
            terminator: TerminatorRead::NoneRead,
            n: 0,
            state: ParseState::ReadMetaInfo(MetaInfoKind::ContentLength),
        }
    }
    pub fn process_data<T: Read>(
        &mut self,
        rd: &mut T,
        out: &mut [u8],
    ) -> Result<usize, std::io::Error> {
        let mut bytes_written_to_out = 0;
        loop {
            match self.state {
                ParseState::ReadMetaInfo(_) => {
                    let mut read_until_term = [0u8; 4];
                    let n_max = self.guaranteed_read();
                    let n_read = rd.read(&mut read_until_term[0..n_max])?;
                    if n_read == 0 {
                        return Ok(bytes_written_to_out);
                    }
                    let mut iter = read_until_term[0..n_read].into_iter();

                    while let (Some(&byte), ParseState::ReadMetaInfo(x)) = (iter.next(), self.state)
                    {
                        match x {
                            MetaInfoKind::ContentLength => match (byte, self.terminator) {
                                (b'0'..=b'9' | b'A'..=b'F', TerminatorRead::NoneRead) => {
                                    self.n =
                                        self.n * 16 + (byte as char).to_digit(16).unwrap() as usize;
                                    if self.n == 0 {
                                        self.state =
                                            ParseState::ReadMetaInfo(MetaInfoKind::HeaderCRLF);
                                    }
                                }
                                (b'\r', TerminatorRead::NoneRead) => {
                                    self.state = ParseState::ReadMetaInfo(MetaInfoKind::HeaderCRLF);
                                    self.terminator = TerminatorRead::CRRead;
                                }
                                _ => {
                                    return Err(std::io::Error::new(
					    std::io::ErrorKind::InvalidData,
					    format!("Unexpected character while reading CRLF, last byte {}, self: {:?}", byte, self),
					));
                                }
                            },
                            MetaInfoKind::HeaderCRLF
                            | MetaInfoKind::ContentCRLF
                            | MetaInfoKind::FinalCRLF => match (byte, self.terminator) {
                                (b'\n', TerminatorRead::CRRead) => {
                                    self.terminator = TerminatorRead::NoneRead;

                                    if x == MetaInfoKind::FinalCRLF {
                                        self.state = ParseState::Done;
                                        continue;
                                    }

                                    if x == MetaInfoKind::ContentCRLF {
                                        self.state =
                                            ParseState::ReadMetaInfo(MetaInfoKind::ContentLength);
                                        continue;
                                    }

                                    if self.n == 0 {
                                        self.state =
                                            ParseState::ReadMetaInfo(MetaInfoKind::FinalCRLF)
                                    } else {
                                        self.state = ParseState::CopyContent;
                                    }
                                }
                                (b'\r', TerminatorRead::NoneRead) => {
                                    self.terminator = TerminatorRead::CRRead;
                                }
                                _ => {
                                    return Err(std::io::Error::new(
					    std::io::ErrorKind::InvalidData,
					    format!("Unexpected character while reading CRLF, last byte {}, self: {:?}", byte as char, self),
					));
                                }
                            },
                        }
		    }
                }
                ParseState::CopyContent => {
                    let next_bytes_to_out =
                        self.guaranteed_read().min(out.len() - bytes_written_to_out);

                    if next_bytes_to_out == 0 && self.n > 0 {
                        return Ok(bytes_written_to_out);
                    }
		    
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
                terminator: TerminatorRead::NoneRead,
                n: 0,
                state: ParseState::ReadMetaInfo(MetaInfoKind::HeaderCRLF),
            } => 4,
            BodyParseChunked {
                terminator: TerminatorRead::NoneRead,
                n: 0,
                state: ParseState::ReadMetaInfo(MetaInfoKind::ContentLength),
            } => 3,
            BodyParseChunked {
                terminator: _,
                n: _,
                state: ParseState::ReadMetaInfo(_),
            } => 2 - self.terminator as usize,
            BodyParseChunked {
                terminator: TerminatorRead::NoneRead,
                n: _,
                state: ParseState::CopyContent,
            } => 4.min(self.n),
            _ => 1,
        }
    }
}
