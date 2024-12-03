//! Read transactions and blocks from blk.dat files.

use crate::parser::error::{Error, Result};
use crate::parser::reader::BlockchainRead;
use crate::parser::xor::{XorReader, XOR_MASK_LEN};
use bitcoin::io::Cursor;
use bitcoin::{Block, Transaction};
use std::collections::HashMap;
use std::fs::{DirEntry, File};
use std::io::{Seek, SeekFrom};
use std::path::{Path, PathBuf};

// the size of a header is 80.
const HEADER_SIZE: u64 = 80;

/// Resolve symlink.
fn resolve_path(entry: &DirEntry) -> std::io::Result<PathBuf> {
    if entry.file_type()?.is_symlink() {
        std::fs::read_link(entry.path())
    } else {
        Ok(entry.path())
    }
}

/// Extract index from block file name.
///
/// For example, return `Some(0)` for `blk00000.dat`.
fn parse_blk_index(path: impl AsRef<Path>) -> Option<i32> {
    let file_name = path.as_ref().file_name().and_then(|f| f.to_str())?;
    let s = file_name.strip_prefix("blk")?;
    let blk_index = s.strip_suffix(".dat")?;
    blk_index.parse::<i32>().ok()
}

/// Scan `blocks` folder to build an index of all blk files.
fn scan_blocks_dir(blocks_dir: &Path) -> Result<HashMap<i32, PathBuf>> {
    let mut blk_files = HashMap::with_capacity(5000);
    for entry in std::fs::read_dir(blocks_dir)? {
        let path = resolve_path(&entry?)?;
        if !path.is_file() {
            continue;
        };

        if let Some(index) = parse_blk_index(path.as_path()) {
            blk_files.insert(index, path);
        }
    }
    blk_files.shrink_to_fit();
    if blk_files.is_empty() {
        Err(Error::EmptyBlockFiles)
    } else {
        Ok(blk_files)
    }
}

/// Reads the block XOR mask.
///
/// If no `xor.dat` file is present, use all-zeroed array to perform an XOR no-op.
///
/// Note: `xor.data` was added since Bitcoin Core 28.0.
fn read_xor_mask<P: AsRef<Path>>(blocks_dir: P) -> std::io::Result<Option<[u8; XOR_MASK_LEN]>> {
    use std::io::Read;
    let path = blocks_dir.as_ref().join("xor.dat");
    if !path.exists() {
        return Ok(Default::default());
    }
    let mut file = File::open(path)?;
    let mut buf = [0_u8; XOR_MASK_LEN];
    file.read_exact(&mut buf)?;
    Ok(Some(buf))
}

/// An index of all blk files found.
#[derive(Debug, Clone)]
pub struct BlkFile {
    files: HashMap<i32, PathBuf>,
    xor_mask: Option<[u8; XOR_MASK_LEN]>,
}

impl BlkFile {
    /// Construct an index of all blk files.
    ///
    /// # Arguments
    ///
    /// `path`: Path of `bitcoin_core_data_dir/blocks`.
    pub(crate) fn new(path: &Path) -> Result<BlkFile> {
        let xor_mask = read_xor_mask(path)?;
        Ok(Self {
            files: scan_blocks_dir(path)?,
            xor_mask,
        })
    }

    /// Read a Block from blk file.
    #[inline]
    pub(crate) fn read_raw_block(&self, n_file: i32, offset: u32) -> Result<Vec<u8>> {
        let blk_path = self
            .files
            .get(&n_file)
            .ok_or(Error::BlockFileNotFound(n_file))?;

        let mut r = XorReader::new(File::open(blk_path)?, self.xor_mask);
        r.seek(SeekFrom::Start(offset as u64 - 4))?;
        let block_size = r.read_u32()?;
        let block = r.read_vec_u8(block_size)?;

        Ok(block)
    }

    /// Read a Block from blk file.
    pub(crate) fn read_block(&self, n_file: i32, offset: u32) -> Result<Block> {
        Cursor::new(self.read_raw_block(n_file, offset)?).read_block()
    }

    /// Read a transaction from blk file.
    pub(crate) fn read_transaction(
        &self,
        n_file: i32,
        n_pos: u32,
        n_tx_offset: u32,
    ) -> Result<Transaction> {
        let blk_path = self
            .files
            .get(&n_file)
            .ok_or(Error::BlockFileNotFound(n_file))?;

        let mut r = XorReader::new(File::open(blk_path)?, self.xor_mask);

        r.seek(SeekFrom::Start(
            n_pos as u64 + n_tx_offset as u64 + HEADER_SIZE,
        ))?;

        r.read_transaction()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_blk_index() {
        assert_eq!(0, parse_blk_index("blk00000.dat").unwrap());
        assert_eq!(6, parse_blk_index("blk6.dat").unwrap());
        assert_eq!(1202, parse_blk_index("blk1202.dat").unwrap());
        assert_eq!(13412451, parse_blk_index("blk13412451.dat").unwrap());
        assert!(parse_blk_index("blkindex.dat").is_none());
        assert!(parse_blk_index("invalid.dat").is_none());
    }
}
