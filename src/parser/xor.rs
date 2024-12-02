//! Credit: https://github.com/sumopool/bitcoin-block-parser/pull/4
//!
//! Handles XOR'd Bitcoin-core block data.
//!
//! - See https://github.com/bitcoin/bitcoin/pull/28052

use std::io::{Read, Seek, SeekFrom};

/// XOR mask length. It's the length of file `blocks/xor.dat`.
pub const XOR_MASK_LEN: usize = 8;

/// Transparent reader for XOR'd blk*.dat files.
pub struct XorReader<R: Read> {
    /// Inner reader.
    inner: R,
    /// Stream position. This is expected to be synchronous with [`Seek::stream_position`],
    /// but without a syscall to fetch it.
    pos: u64,
    /// XOR mask if one exists.
    mask: Option<[u8; XOR_MASK_LEN]>,
    buffer: Vec<u8>,
}

impl<R: Read> XorReader<R> {
    /// Create a reader wrapper that performs XOR on reads.
    pub fn new(reader: R, xor_mask: Option<[u8; XOR_MASK_LEN]>) -> Self {
        Self {
            inner: reader,
            pos: 0,
            mask: xor_mask,
            buffer: Vec::new(),
        }
    }
}

impl<R: Read> Read for XorReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let size = self.inner.read(buf)?;
        if let Some(mask) = self.mask {
            for x in &mut buf[..size] {
                *x ^= mask[(self.pos % mask.len() as u64) as usize];
                self.pos += 1;
            }
        }
        Ok(size)
    }
}

impl<R: Read> std::io::BufRead for XorReader<R> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        if self.buffer.is_empty() {
            let mut temp_buf = vec![0; 4096]; // Or some appropriate buffer size
            let n = self.inner.read(&mut temp_buf)?;
            if n == 0 {
                return Ok(&[]); // End of stream
            }
            // Apply XOR mask to the buffer
            if let Some(mask) = &self.mask {
                for i in temp_buf.iter_mut().take(n) {
                    *i ^= mask[(self.pos % mask.len() as u64) as usize];
                    self.pos += 1;
                }
            }
            self.buffer = temp_buf[..n].to_vec(); // Store the modified buffer
        }
        Ok(&self.buffer) // Return the current buffer
    }

    fn consume(&mut self, amt: usize) {
        self.buffer.drain(..amt);
    }
}

impl<R: Seek + Read> Seek for XorReader<R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let result = self.inner.seek(pos);
        // Just use a syscall to update the current position.
        self.pos = self.inner.stream_position()?;
        result
    }
}

impl<R: Read> bitcoin::io::Read for XorReader<R> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> bitcoin::io::Result<usize> {
        Ok(std::io::Read::read(self, buf)?)
    }
}

impl<R: Read> bitcoin::io::BufRead for XorReader<R> {
    #[inline]
    fn fill_buf(&mut self) -> bitcoin::io::Result<&[u8]> {
        Ok(std::io::BufRead::fill_buf(self)?)
    }

    #[inline]
    fn consume(&mut self, amount: usize) {
        std::io::BufRead::consume(self, amount)
    }
}
