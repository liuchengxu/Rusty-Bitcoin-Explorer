//! Defines binary file reading utilities for Bitcoin blockchain objects.

use super::xor::XorReader;
use crate::parser::error::Result;
use crate::BlockHeader;
use bitcoin::consensus::Decodable;
use bitcoin::io::Cursor;
use bitcoin::{Block, Transaction};
use byteorder::{ByteOrder, LittleEndian};
use std::fs::File;
use std::io::BufReader;

/// A trait for reading Bitcoin blockchain data from binary sources.
///
/// This trait provides methods for reading various types of data from a stream, including
/// variable-length integers, blocks, transactions, and headers. Implementations are provided
/// for common types like `Cursor` and `BufReader`, making it easier to read Bitcoin data from
/// different sources.
pub trait BlockchainRead: bitcoin::io::BufRead {
    /// Reads a variable-length integer (varint) from the stream.
    /// This method reads a sequence of bytes and decodes it into a usize.
    ///
    /// The varint encoding uses a 7-bit encoding for the integer, and the highest bit of each byte
    /// signals whether more bytes follow.
    ///
    /// # Returns
    /// `Result<usize>`: The decoded integer, or an error if the read fails.
    #[inline]
    fn read_varint(&mut self) -> Result<usize> {
        let mut n = 0;
        loop {
            let ch_data = self.read_u8()?;
            n = (n << 7) | (ch_data & 0x7F) as usize;
            if ch_data & 0x80 > 0 {
                n += 1;
            } else {
                break;
            }
        }
        Ok(n)
    }

    /// Reads a single byte from the stream.
    ///
    /// # Returns
    /// `Result<u8>`: The byte read from the stream.
    #[inline]
    fn read_u8(&mut self) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    /// Reads a 256-bit value (32 bytes) from the stream, typically used for hashes.
    #[inline]
    fn read_u256(&mut self) -> Result<[u8; 32]> {
        let mut buf = [0u8; 32];
        self.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Reads a 32-bit unsigned integer from the stream.
    #[inline]
    fn read_u32(&mut self) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.read_exact(&mut buf)?;
        Ok(LittleEndian::read_u32(&buf))
    }

    /// Reads a 32-bit signed integer from the stream.
    #[inline]
    fn read_i32(&mut self) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.read_exact(&mut buf)?;
        Ok(LittleEndian::read_i32(&buf))
    }

    /// Reads a vector of `u8` bytes from the stream with a specified count.
    ///
    /// # Arguments
    /// * `count`: The number of bytes to read.
    ///
    /// # Returns
    /// `Result<Vec<u8>>`: The vector containing the read bytes.
    #[inline]
    fn read_vec_u8(&mut self, count: u32) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; count as usize];
        self.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Reads a complete block from the stream.
    #[inline]
    fn read_block(&mut self) -> Result<Block> {
        Ok(Block::consensus_decode(self)?)
    }

    /// Reads a complete transaction from the stream.
    #[inline]
    fn read_transaction(&mut self) -> Result<Transaction> {
        Ok(Transaction::consensus_decode(self)?)
    }

    /// Reads a complete block header from the stream.
    #[inline]
    fn read_block_header(&mut self) -> Result<BlockHeader> {
        Ok(BlockHeader::consensus_decode(self)?)
    }
}

impl BlockchainRead for Cursor<&[u8]> {}
impl BlockchainRead for Cursor<Vec<u8>> {}
impl BlockchainRead for BufReader<File> {}
impl BlockchainRead for XorReader<File> {}
