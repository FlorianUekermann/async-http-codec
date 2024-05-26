use std::io::Read;

enum ReadState {
    ReadLength,
    ReadLine,
}

struct BodyParseChunked {
    overlap: u32,
    n: u32,
    state: ReadState,
}

impl BodyParseChunked {
    pub fn new() -> Self {
	BodyParseChunked { overlap: 0, n: 0, state: ReadState::ReadLine }	
    }
    pub fn process_data<T: Read>(&mut self, rd: &mut T, out: &mut [u8])  -> Result<usize, std::io::Error> {
	let mut read_until_term = [0u8;3];
	loop {
	    match self.state {
		ReadState::ReadLength => {
		    let mut bytes_read = 0;
		    
		    let next_bytes = self.guaranteed_read();
		    if next_bytes == 0 {
			self.overlap = 0;
			self.state = ReadState::ReadLine;
			return Ok(bytes_read)
		    }
		    
		    rd.read_exact(&mut read_until_term[0..next_bytes])?;
		    let mut iter = read_until_term.into_iter();
	    
		    while let Some(byte) = iter.next() {
			bytes_read += 1;
			
			match byte as char {
			    '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' | '0' | 'A' | 'B' | 'C' | 'D' | 'E' | 'F' => {
				self.n = self.n*10 + (byte as char).to_digit(10).unwrap();
			    }
			    '\r' => {
				match iter.next()  {
				    // 10u8 == '\n' as u8
				    Some(10u8) => {
					bytes_read += 1;
					self.overlap = 2;
				    },
				    None => {
					self.overlap = 1;
				    },
				    _ => {
					return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Error after \r while reading Content-Length Header"));
				    }
				}
			    },
			    '\n' if self.overlap == 1 => {
				self.overlap = 2;
			    },
			    _ => {
				return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Unexpected character in Content-Length Header"));
			    },
			}
		    }
		}
		ReadState::ReadLine => {
		    rd.read_exact(&mut out[0..self.n as usize])?;
		    let next_bytes = self.guaranteed_read();
		    rd.read_exact(&mut read_until_term[0..next_bytes])?;
		    match &read_until_term[0..2] {
			// 13u8 == '\r' as u8
			// 10u8 == '\n' as u8
			[13u8, 10u8] => {
			    self.state = ReadState::ReadLength;
			    return Ok(self.n as usize + 2);
			}
			_ => {
			    return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "No CRLF after reading Content"));			    
			}
		    }
		}
	    }
	}
    }
    /// returns min number of unprocessed bytes (0 if done)
    pub fn guaranteed_read(&self) -> usize {
	match self {
	    BodyParseChunked { overlap: 0, n: 0, state: ReadState::ReadLine } => { 3 }
	    BodyParseChunked { overlap: 0, n: _, state: ReadState::ReadLine } => { 2 }
	    BodyParseChunked { overlap: 2, n: _, state: ReadState::ReadLine } => { 0 }
	    BodyParseChunked { overlap: _, n: _, state: ReadState::ReadLength } => { self.n as usize }
	    _ => { 1 }
	}
    }
}
