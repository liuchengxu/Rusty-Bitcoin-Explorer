//! Read block index in memory from levelDB.

use crate::parser::error::Result;
use crate::parser::reader::BlockchainRead;
use crate::BlockHeader;
use bitcoin::io::Cursor;
use bitcoin::BlockHash;
use leveldb::database::iterator::LevelDBIterator;
use leveldb::database::Database;
use leveldb::iterator::Iterable;
use leveldb::options::{Options, ReadOptions};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::Path;

/// See Bitcoin Core repository for definition.
const BLOCK_VALID_HEADER: u32 = 1;
const BLOCK_VALID_TREE: u32 = 2;
const BLOCK_VALID_TRANSACTIONS: u32 = 3;
const BLOCK_VALID_CHAIN: u32 = 4;
const BLOCK_VALID_SCRIPTS: u32 = 5;
const BLOCK_VALID_MASK: u32 = BLOCK_VALID_HEADER
    | BLOCK_VALID_TREE
    | BLOCK_VALID_TRANSACTIONS
    | BLOCK_VALID_CHAIN
    | BLOCK_VALID_SCRIPTS;
const BLOCK_HAVE_DATA: u32 = 8;
const BLOCK_HAVE_UNDO: u32 = 16;

/// BLOCK_INDEX RECORD as defined in Bitcoin Core.
#[derive(Serialize, Clone)]
pub struct BlockIndexRecord {
    pub n_version: i32,
    pub n_height: i32,
    pub n_status: u32,
    pub n_tx: u32,
    pub n_file: i32,
    pub n_data_pos: u32,
    pub n_undo_pos: u32,
    pub block_header: BlockHeader,
}

impl BlockIndexRecord {
    pub fn is_valid(&self) -> bool {
        self.n_height == 0
            || (self.n_status & BLOCK_VALID_MASK >= BLOCK_VALID_SCRIPTS
                && self.n_status & BLOCK_HAVE_DATA > 0)
    }

    /// Decode levelDB value for Block Index Record.
    ///
    /// https://github.com/bitcoin/bitcoin/blob/0903ce8dbc25d3823b03d52f6e6bff74d19e801e/src/chain.h#L377
    fn decode(values: &[u8]) -> Result<Self> {
        let mut reader = Cursor::new(values);

        let n_version = reader.read_varint()? as i32;
        let n_height = reader.read_varint()? as i32;
        let n_status = reader.read_varint()? as u32;
        let n_tx = reader.read_varint()? as u32;
        let n_file = if n_status & (BLOCK_HAVE_DATA | BLOCK_HAVE_UNDO) > 0 {
            reader.read_varint()? as i32
        } else {
            -1
        };
        let n_data_pos = if n_status & BLOCK_HAVE_DATA > 0 {
            reader.read_varint()? as u32
        } else {
            u32::MAX
        };
        let n_undo_pos = if n_status & BLOCK_HAVE_UNDO > 0 {
            reader.read_varint()? as u32
        } else {
            u32::MAX
        };
        let block_header = reader.read_block_header()?;

        Ok(Self {
            n_version,
            n_height,
            n_status,
            n_tx,
            n_file,
            n_data_pos,
            n_undo_pos,
            block_header,
        })
    }
}

impl fmt::Debug for BlockIndexRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BlockIndexRecord")
            .field("version", &self.n_version)
            .field("height", &self.n_height)
            .field("status", &self.n_status)
            .field("n_tx", &self.n_tx)
            .field("n_file", &self.n_file)
            .field("n_data_pos", &self.n_data_pos)
            .field("header", &self.block_header)
            .finish()
    }
}

#[derive(Clone)]
pub struct BlockIndex {
    /// List of all block records.
    pub records: Box<[BlockIndexRecord]>,
    /// Map from block hash to block height.
    pub hash_to_height: HashMap<BlockHash, i32>,
}

impl BlockIndex {
    /// Build a collections of block index.
    pub(crate) fn new(p: impl AsRef<Path>) -> Result<BlockIndex> {
        let records = load_block_index(p.as_ref())?.into_boxed_slice();

        // build a reverse index to lookup block height of a particular block hash.
        let mut hash_to_height = HashMap::with_capacity(records.len());
        for b in records.iter() {
            hash_to_height.insert(b.block_header.block_hash(), b.n_height);
        }
        hash_to_height.shrink_to_fit();
        Ok(BlockIndex {
            records,
            hash_to_height,
        })
    }
}

#[inline]
fn is_block_index_record(data: &[u8]) -> bool {
    data.first() == Some(&b'b')
}

/// Load all block index in memory from leveldb (i.e. `blocks/index` path).
///
/// Map from block height to block index record.
fn load_block_index(path: &Path) -> Result<Vec<BlockIndexRecord>> {
    log::debug!("Start loading block_index");

    let mut options = Options::new();
    options.create_if_missing = false;
    let db: Database<BlockKey> = Database::open(path, options)?;

    let mut block_index_by_block_hash = BTreeMap::new();
    let mut max_height_block_hash = Option::<(BlockHash, i32)>::None;

    let mut iter = db.iter(ReadOptions::new());

    while iter.advance() {
        let k = iter.key();
        let v = iter.value();
        if is_block_index_record(&k.key) {
            let record = BlockIndexRecord::decode(&v)?;
            // only add valid block index record that has block data.
            if record.is_valid() {
                let block_hash = record.block_header.block_hash();
                // find the block with max height
                if let Some((hash, height)) = max_height_block_hash.as_mut() {
                    if record.n_height > *height {
                        *hash = block_hash;
                        *height = record.n_height;
                    }
                } else {
                    max_height_block_hash = Some((block_hash, record.n_height));
                }
                block_index_by_block_hash.insert(block_hash, record);
            }
        }
    }

    // build the longest chain
    if let Some((hash, height)) = max_height_block_hash {
        let mut block_index = Vec::with_capacity(height as usize + 1);
        let mut current_hash = hash;
        let mut current_height = height;
        // recursively build block index from max height block.
        while current_height >= 0 {
            let blk = block_index_by_block_hash
                .remove(&current_hash)
                .expect("block hash not found in block index!");
            assert_eq!(
                current_height, blk.n_height,
                "some block info missing from block index levelDB,\
                       delete Bitcoin folder and re-download!"
            );
            current_hash = blk.block_header.prev_blockhash;
            current_height -= 1;
            block_index.push(blk);
        }
        block_index.reverse();
        Ok(block_index)
    } else {
        Ok(Vec::new())
    }
}

/// levelDB key util
struct BlockKey {
    key: Vec<u8>,
}

impl db_key::Key for BlockKey {
    fn from_u8(key: &[u8]) -> Self {
        Self {
            key: Vec::from(key),
        }
    }

    fn as_slice<T, F: Fn(&[u8]) -> T>(&self, f: F) -> T {
        f(&self.key)
    }
}
