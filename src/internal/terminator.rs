pub struct TerminatorOverlap<'a> {
    terminator: &'a [u8],
    overlap: usize,
}

impl<'a> TerminatorOverlap<'a> {
    pub fn new(terminator: &'a [u8]) -> TerminatorOverlap {
        TerminatorOverlap {
            terminator,
            overlap: 0,
        }
    }
    /// returns min number of unprocessed bytes (remaining unmatched terminator bytes)
    pub fn remaining(&self) -> usize {
        self.terminator.len() - self.overlap
    }
    /// scans data for overlap with remaining terminator bytes
    pub fn process(&mut self, data: &[u8]) {
        match self.overlap {
            0 => {
                for i in 0..data.len() {
                    let window_size = self.terminator.len() - i;
                    if data[i..] == self.terminator[0..window_size] {
                        self.overlap = window_size;
                    }
                }
            }
            x => match data[0..(self.terminator.len() - x)] == self.terminator[x..] {
                true => self.overlap = self.terminator.len(),
                false => self.overlap = 0,
            },
        }
    }
    /// true if complete terminator was processed
    pub fn done(&self) -> bool {
        self.overlap == self.terminator.len()
    }
    /// slice read buffer to maximum size guaranteed to not read past the terminator
    pub fn max_read_buf(&self, buf: &'a mut [u8]) -> &'a mut [u8] {
        let len = buf.len();
        &mut buf[0..len.min(self.remaining())]
    }
}
