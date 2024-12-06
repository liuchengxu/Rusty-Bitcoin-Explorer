//! On disk transaction index database

use crate::parser::block_index::BlockIndex;
use crate::parser::error::{Error, Result};
use crate::parser::reader::BlockchainRead;
use bitcoin::hashes::Hash;
use bitcoin::io::Cursor;
use bitcoin::Txid;
use leveldb::database::Database;
use leveldb::kv::KV;
use leveldb::options::{Options, ReadOptions};
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

const GENESIS_TXID: &str = "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b";

/// Represents the disk location of a transaction.
pub struct TransactionPosition {
    pub txid: Txid,
    pub n_file: i32,
    pub n_data_pos: u32,
    pub n_tx_offset: u32,
}

/// Responsible for looking up transaction position using txid.
///
/// This requires setting `txindex=1` in Bitcoin Core.
pub struct TxDB {
    db: Database<TxKey>,
    // used for reverse looking up to block height
    file_pos_to_height: BTreeMap<(i32, u32), i32>,
    genesis_txid: Txid,
}

impl TxDB {
    /// Initialize TxDB for transaction queries.
    pub fn open(path: &Path, blk_index: &BlockIndex) -> Option<Self> {
        if !path.exists() {
            log::warn!(
                "Failed to open tx_index DB: {} does not exist",
                path.display()
            );
            return None;
        }

        let options = Options::new();

        match Database::open(path, options) {
            Ok(db) => {
                log::debug! {"Successfully opened tx_index DB!"}
                let file_pos_to_height: BTreeMap<_, _> = blk_index
                    .records
                    .iter()
                    .map(|b| ((b.n_file, b.n_data_pos), b.n_height))
                    .collect();
                Some(Self {
                    db,
                    file_pos_to_height,
                    genesis_txid: Txid::from_str(GENESIS_TXID)
                        .expect("Genesis txid must be valid; qed"),
                })
            }
            Err(e) => {
                log::warn!("Failed to open tx_index DB: {:?}", e);
                None
            }
        }
    }

    /// genesis tx is not included in UTXO because of Bitcoin Core Bug
    #[inline]
    pub(crate) fn is_genesis_tx(&self, txid: Txid) -> bool {
        txid == self.genesis_txid
    }

    /// Returns the disk location of the transaction data with given txid.
    ///
    /// Note that this function cannot support genesis txid.
    pub(crate) fn get_tx_position(&self, txid: Txid) -> Result<TransactionPosition> {
        let inner = txid.as_byte_array();
        let mut key = Vec::with_capacity(inner.len() + 1);
        key.push(b't');
        key.extend(inner);
        let key = TxKey { key };
        let value = self.db.get(ReadOptions::new(), &key)?;
        if let Some(value) = value {
            // Decode the transaction record from database.
            //
            // https://github.com/bitcoin/bitcoin/blob/0903ce8dbc25d3823b03d52f6e6bff74d19e801e/src/index/txindex.cpp#L63
            let mut reader = Cursor::new(value);
            let tx_record = TransactionPosition {
                txid,
                n_file: reader.read_varint()? as i32,
                n_data_pos: reader.read_varint()? as u32,
                n_tx_offset: reader.read_varint()? as u32,
            };
            Ok(tx_record)
        } else {
            Err(Error::TransactionRecordNotFound(txid))
        }
    }

    pub(crate) fn get_block_height(&self, txid: Txid) -> Result<usize> {
        if self.is_genesis_tx(txid) {
            return Ok(0);
        }
        let tx_pos = self.get_tx_position(txid)?;
        match self
            .file_pos_to_height
            .get(&(tx_pos.n_file, tx_pos.n_data_pos))
        {
            Some(height) => Ok(*height as usize),
            None => Err(Error::CannotFindHeightForTransaction(txid)),
        }
    }
}

/// levelDB key utility
struct TxKey {
    key: Vec<u8>,
}

/// levelDB key utility
impl db_key::Key for TxKey {
    fn from_u8(key: &[u8]) -> Self {
        Self {
            key: Vec::from(key),
        }
    }

    fn as_slice<T, F: Fn(&[u8]) -> T>(&self, f: F) -> T {
        f(&self.key)
    }
}
