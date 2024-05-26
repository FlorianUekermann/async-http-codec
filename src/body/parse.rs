use std::io::Read;

enum ReadState {
    ReadLength,
    ReadLine,
}

struct BodyParseChunked {
    overlap: usize,
    n: usize,
    state: ReadState,
}

impl BodyParseChunked {
    pub fn new() -> Self {
	BodyParseChunked { overlap: 0, n: 0, state: ReadState::ReadLength }	
    }
    pub fn process_data<T: Read>(&mut self, rd: &mut T, out: &mut [u8])  -> Result<usize, std::io::Error> {
	let mut read_until_term = [0u8;3];
	let out_len = out.len();
	loop {
	    match self.state {
		ReadState::ReadLength => {
		    let next_bytes = self.guaranteed_read();
		    if next_bytes == 0 {
			self.overlap = 0;
			self.state = ReadState::ReadLine;
			continue;
		    }
		    
		    let n_max = rd.read(&mut read_until_term[0..next_bytes])?;
		    let mut iter = read_until_term[0..n_max].into_iter();
	    
		    while let Some(&byte) = iter.next() {
			match byte as char {
			    '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' | '0' | 'A' | 'B' | 'C' | 'D' | 'E' | 'F' if self.overlap == 0 => {
				self.n = self.n*16 + (byte as char).to_digit(16).unwrap() as usize;
			    }
			    '\r' => {
				self.overlap = 1;
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
		    
		    let next_bytes = self.guaranteed_read().min(self.n);
		    let bytes_read = rd.read(&mut out[0..next_bytes.min(out_len) as usize])?;
		    self.n -= bytes_read;

		    if self.n == 0 {
		    
			let next_bytes = self.guaranteed_read();
			let n_reads = rd.read(&mut read_until_term[0..next_bytes])?;
			let mut iter = read_until_term[0..n_reads].into_iter();

			while let Some(&byte) = iter.next()  {
			    match byte as char {
				'\r' => self.overlap = 1,
				'\n' if self.overlap == 1 => {
				   *self = BodyParseChunked::new();
				}
				_ => {
				    return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "No CRLF after reading Content"));
				}
			    }
			}
		    }
		    return Ok(bytes_read as usize);
		}
	    }
	}
    }
    /// returns min number of unprocessed bytes (0 if done)
    pub fn guaranteed_read(&self) -> usize {
	match self {
	    BodyParseChunked { overlap: 0, n: 0, state: ReadState::ReadLength } => { 3 }
	    BodyParseChunked { overlap: 0, n: _, state: ReadState::ReadLength } => { 2 }
	    BodyParseChunked { overlap: 2, n: _, state: ReadState::ReadLength } => { 0 }
	    BodyParseChunked { overlap: _, n: _, state: ReadState::ReadLine } => { self.n + 2 - self.overlap}
	    _ => { 1 }
	}
    }
}
