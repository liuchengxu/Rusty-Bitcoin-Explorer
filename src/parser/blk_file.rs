//! Read transactions and blocks from blk.dat files.

use crate::parser::errors::{OpError, OpErrorKind, OpResult};
use crate::parser::reader::BlockchainRead;
use crate::parser::xor::{XorReader, XOR_MASK_LEN};
use bitcoin::io::Cursor;
use bitcoin::{Block, Transaction};
use std::collections::HashMap;
use std::convert::From;
use std::fs::{self, DirEntry, File};
use std::io::{self, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Resolve symlink.
fn resolve_path(entry: &DirEntry) -> io::Result<PathBuf> {
    if entry.file_type()?.is_symlink() {
        fs::read_link(entry.path())
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

/// Scan blk folder to build an index of all blk files.
fn scan_path(path: &Path) -> OpResult<HashMap<i32, PathBuf>> {
    let mut collected = HashMap::with_capacity(4000);
    for entry in fs::read_dir(path)? {
        match entry {
            Ok(de) => {
                let path = resolve_path(&de)?;
                if !path.is_file() {
                    continue;
                };

                if let Some(index) = parse_blk_index(path.as_path()) {
                    collected.insert(index, path);
                }
            }
            Err(msg) => {
                return Err(OpError::from(msg));
            }
        }
    }
    collected.shrink_to_fit();
    if collected.is_empty() {
        Err(OpError::new(OpErrorKind::RuntimeError).join_msg("No blk files found!"))
    } else {
        Ok(collected)
    }
}

/// Reads the block XOR mask.
///
/// If no `xor.dat` file is present, use all-zeroed array to perform an XOR no-op.
///
/// Note: added since Bitcoin Core 28.0.
fn read_xor_mask<P: AsRef<Path>>(dir: P) -> std::io::Result<Option<[u8; XOR_MASK_LEN]>> {
    use std::io::Read;
    let path = dir.as_ref().join("xor.dat");
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
    pub(crate) fn new(path: &Path) -> OpResult<BlkFile> {
        let xor_mask = read_xor_mask(path)?;
        Ok(Self {
            files: scan_path(path)?,
            xor_mask,
        })
    }

    /// Read a Block from blk file.
    #[inline]
    pub(crate) fn read_raw_block(&self, n_file: i32, offset: u32) -> OpResult<Vec<u8>> {
        let blk_path = self
            .files
            .get(&n_file)
            .ok_or(OpError::from("blk file not found, sync with bitcoin core"))?;

        let mut r = XorReader::new(File::open(blk_path)?, self.xor_mask);
        r.seek(SeekFrom::Start(offset as u64 - 4))?;
        let block_size = r.read_u32()?;
        let block = r.read_u8_vec(block_size)?;

        Ok(block)
    }

    /// Read a Block from blk file.
    pub(crate) fn read_block(&self, n_file: i32, offset: u32) -> OpResult<Block> {
        Cursor::new(self.read_raw_block(n_file, offset)?).read_block()
    }

    /// Read a transaction from blk file.
    pub(crate) fn read_transaction(
        &self,
        n_file: i32,
        n_pos: u32,
        n_tx_offset: u32,
    ) -> OpResult<Transaction> {
        let blk_path = self
            .files
            .get(&n_file)
            .ok_or(OpError::from("blk file not found, sync with bitcoin core"))?;

        let mut r = XorReader::new(File::open(blk_path)?, self.xor_mask);
        // the size of a header is 80.
        r.seek(SeekFrom::Start(n_pos as u64 + n_tx_offset as u64 + 80))?;
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
