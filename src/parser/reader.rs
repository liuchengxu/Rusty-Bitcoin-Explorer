//! Define binary file readers.

use super::xor::XorReader;
use crate::parser::error::Result;
use crate::BlockHeader;
use bitcoin::consensus::Decodable;
use bitcoin::io::Cursor;
use bitcoin::{Block, Transaction};
use byteorder::{ByteOrder, LittleEndian};
use std::fs::File;
use std::io::BufReader;

/// Binary file read utilities.
pub trait BlockchainRead: bitcoin::io::BufRead {
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

    #[inline]
    fn read_u8(&mut self) -> Result<u8> {
        let mut slice = [0u8; 1];
        self.read_exact(&mut slice)?;
        Ok(slice[0])
    }

    #[inline]
    fn read_u256(&mut self) -> Result<[u8; 32]> {
        let mut arr = [0u8; 32];
        self.read_exact(&mut arr)?;
        Ok(arr)
    }

    #[inline]
    fn read_u32(&mut self) -> Result<u32> {
        let mut arr = [0u8; 4];
        self.read_exact(&mut arr)?;
        let u = LittleEndian::read_u32(&arr);
        Ok(u)
    }

    #[inline]
    fn read_i32(&mut self) -> Result<i32> {
        let mut arr = [0u8; 4];
        self.read_exact(&mut arr)?;
        let u = LittleEndian::read_i32(&arr);
        Ok(u)
    }

    #[inline]
    fn read_u8_vec(&mut self, count: u32) -> Result<Vec<u8>> {
        let mut arr = vec![0u8; count as usize];
        self.read_exact(&mut arr)?;
        Ok(arr)
    }

    #[inline]
    fn read_block(&mut self) -> Result<Block> {
        Ok(Block::consensus_decode(self)?)
    }

    #[inline]
    fn read_transaction(&mut self) -> Result<Transaction> {
        Ok(Transaction::consensus_decode(self)?)
    }

    #[inline]
    fn read_block_header(&mut self) -> Result<BlockHeader> {
        Ok(BlockHeader::consensus_decode(self)?)
    }
}

impl BlockchainRead for Cursor<&[u8]> {}
impl BlockchainRead for Cursor<Vec<u8>> {}
impl BlockchainRead for BufReader<File> {}
impl BlockchainRead for XorReader<File> {}
