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

///
/// An index of all blk files found.
///
#[derive(Debug, Clone)]
pub struct BlkFile {
    files: HashMap<i32, PathBuf>,
    xor_mask: Option<[u8; XOR_MASK_LEN]>,
}

impl BlkFile {
    ///
    /// Construct an index of all blk files.
    ///
    pub(crate) fn new(path: &Path) -> OpResult<BlkFile> {
        let xor_mask = read_xor_mask(path)?;
        Ok(BlkFile {
            files: BlkFile::scan_path(path)?,
            xor_mask,
        })
    }

    ///
    /// Read a Block from blk file.
    ///
    #[inline]
    pub(crate) fn read_raw_block(&self, n_file: i32, offset: u32) -> OpResult<Vec<u8>> {
        if let Some(blk_path) = self.files.get(&n_file) {
            let mut r = XorReader::new(File::open(blk_path)?, self.xor_mask);
            r.seek(SeekFrom::Start(offset as u64 - 4))?;
            let block_size = r.read_u32()?;
            let block = r.read_u8_vec(block_size)?;
            Ok(block)
        } else {
            Err(OpError::from("blk file not found, sync with bitcoin core"))
        }
    }

    ///
    /// Read a Block from blk file.
    ///
    pub(crate) fn read_block(&self, n_file: i32, offset: u32) -> OpResult<Block> {
        Cursor::new(self.read_raw_block(n_file, offset)?).read_block()
    }

    ///
    /// Read a transaction from blk file.
    ///
    pub(crate) fn read_transaction(
        &self,
        n_file: i32,
        n_pos: u32,
        n_tx_offset: u32,
    ) -> OpResult<Transaction> {
        if let Some(blk_path) = self.files.get(&n_file) {
            let mut r = XorReader::new(File::open(blk_path)?, self.xor_mask);
            // the size of a header is 80.
            r.seek(SeekFrom::Start(n_pos as u64 + n_tx_offset as u64 + 80))?;
            r.read_transaction()
        } else {
            Err(OpError::from("blk file not found, sync with bitcoin core"))
        }
    }

    ///
    /// Scan blk folder to build an index of all blk files.
    ///
    fn scan_path(path: &Path) -> OpResult<HashMap<i32, PathBuf>> {
        let mut collected = HashMap::with_capacity(4000);
        for entry in fs::read_dir(path)? {
            match entry {
                Ok(de) => {
                    let path = BlkFile::resolve_path(&de)?;
                    if !path.is_file() {
                        continue;
                    };
                    if let Some(file_name) = path.as_path().file_name() {
                        if let Some(file_name) = file_name.to_str() {
                            if let Some(index) = BlkFile::parse_blk_index(file_name) {
                                collected.insert(index, path);
                            }
                        }
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

    ///
    /// Resolve symlink.
    ///
    fn resolve_path(entry: &DirEntry) -> io::Result<PathBuf> {
        if entry.file_type()?.is_symlink() {
            fs::read_link(entry.path())
        } else {
            Ok(entry.path())
        }
    }

    ///
    /// Extract index from block file name.
    ///
    fn parse_blk_index(file_name: &str) -> Option<i32> {
        let prefix = "blk";
        let ext = ".dat";
        if file_name.starts_with(prefix) && file_name.ends_with(ext) {
            file_name[prefix.len()..(file_name.len() - ext.len())]
                .parse::<i32>()
                .ok()
        } else {
            None
        }
    }
}

/// Reads the block XOR mask. If no `xor.dat` file is present,
/// use all-zeroed array to perform an XOR no-op.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_blk_index() {
        assert_eq!(0, BlkFile::parse_blk_index("blk00000.dat").unwrap());
        assert_eq!(6, BlkFile::parse_blk_index("blk6.dat").unwrap());
        assert_eq!(1202, BlkFile::parse_blk_index("blk1202.dat").unwrap());
        assert_eq!(
            13412451,
            BlkFile::parse_blk_index("blk13412451.dat").unwrap()
        );
        assert!(BlkFile::parse_blk_index("blkindex.dat").is_none());
        assert!(BlkFile::parse_blk_index("invalid.dat").is_none());
    }
}
