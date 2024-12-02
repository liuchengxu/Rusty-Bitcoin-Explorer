//! Crates APIs, essential structs, functions, methods are all here!
//!
//! To quickly understand how to use this crate, have a look at the
//! documentation for `bitcoin_explorer::BitcoinDB`!!.
//!
//! # Example
//!
//! ```rust
//! use bitcoin_explorer::BitcoinDB;
//! use std::path::Path;
//!
//! let path = Path::new("/Users/me/bitcoin");
//!
//! // launch without reading txindex
//! let db = BitcoinDB::new(path, false).unwrap();
//!
//! // launch attempting to read txindex
//! let db = BitcoinDB::new(path, true).unwrap();
//! ```

use crate::parser::blk_file::BlkFile;
use crate::parser::error::{Error, Result};
use crate::parser::script::{evaluate_script, ScriptInfo};
use crate::parser::tx_index::TxDB;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

// re-exports
pub use crate::iter::{BlockIter, ConnectedBlockIter};
pub use crate::parser::block_index::{BlockIndex, BlockIndexRecord};
pub use crate::parser::proto::connected_proto::{
    ConnectedBlock, ConnectedTx, FConnectedBlock, FConnectedTransaction, SConnectedBlock,
    SConnectedTransaction,
};
pub use crate::parser::proto::full_proto::{FBlock, FBlockHeader, FTransaction, FTxOut};
pub use crate::parser::proto::simple_proto::{SBlock, SBlockHeader, STransaction, STxOut};
pub use bitcoin::blockdata::block::Header as BlockHeader;
pub use bitcoin::hashes::hex::FromHex;
pub use bitcoin::{Address, Block, BlockHash, Network, Script, ScriptBuf, Transaction, Txid};

///
/// Extract addresses from a script public key.
///
#[deprecated(since = "1.2.7", note = "use `get_addresses_from_script` instead")]
pub fn parse_script(script_pub_key: &str) -> Result<ScriptInfo> {
    get_addresses_from_script(script_pub_key)
}

///
/// Extract addresses from a script public key.
///
#[inline]
pub fn get_addresses_from_script(script_pub_key: &str) -> Result<ScriptInfo> {
    let script_buf = ScriptBuf::from_hex(script_pub_key)?;
    Ok(evaluate_script(script_buf.as_script(), Network::Bitcoin))
}

pub struct InnerDB {
    pub block_index: BlockIndex,
    pub blk_file: BlkFile,
    pub tx_db: TxDB,
}

///
/// This is the main struct of this crate!! Click and read the doc.
///
/// All queries start from initializing `BitcoinDB`.
///
/// Note: This is an Arc wrap around `InnerDB`.
///
#[derive(Clone)]
pub struct BitcoinDB(Arc<InnerDB>);

impl Deref for BitcoinDB {
    type Target = InnerDB;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl BitcoinDB {
    ///
    /// This is the main structure for reading Bitcoin blockchain data.
    ///
    /// Instantiating this class by passing the `-datadir` directory of
    /// Bitcoin core to the `new()` method.
    /// `tx_index`: whether to try to open tx_index levelDB.
    ///
    /// # Example
    ///
    /// ```rust
    /// use bitcoin_explorer::BitcoinDB;
    /// use std::path::Path;
    ///
    /// let path = Path::new("/Users/me/bitcoin");
    ///
    /// // launch without reading txindex
    /// let db = BitcoinDB::new(path, false).unwrap();
    ///
    /// // launch attempting to read txindex
    /// let db = BitcoinDB::new(path, true).unwrap();
    /// ```
    pub fn new(p: &Path, tx_index: bool) -> Result<BitcoinDB> {
        if !p.exists() {
            return Err(Error::BitcoinDataDirectoryDoesNotExist(
                p.display().to_string(),
            ));
        }
        let blk_path = p.join("blocks");
        let index_path = blk_path.join("index");
        let block_index = BlockIndex::new(index_path.as_path())?;
        let tx_db = if tx_index {
            let tx_index_path = p.join("indexes").join("txindex");
            TxDB::new(&tx_index_path, &block_index)
        } else {
            TxDB::null()
        };
        let inner = InnerDB {
            block_index,
            blk_file: BlkFile::new(blk_path.as_path())?,
            tx_db,
        };
        Ok(BitcoinDB(Arc::new(inner)))
    }

    ///
    /// Get the maximum height found in block index.
    ///
    /// Note, not all blocks lower than this height have
    /// been downloaded (different from `get_block_count()`).
    ///
    /// Deprecated: use `get_block_count()`
    ///
    #[deprecated(since = "1.2.6", note = "use `get_block_count()` instead")]
    pub fn get_max_height(&self) -> usize {
        self.block_index.records.len()
    }

    ///
    /// Get the maximum number of blocks downloaded.
    ///
    /// This API guarantee that block 0 to `get_block_count() - 1`
    /// have been downloaded and available for query.
    ///
    pub fn get_block_count(&self) -> usize {
        let records = self.block_index.records.len();
        for h in 0..records {
            // n_tx == 0 indicates that the block is not downloaded
            if self.block_index.records.get(h).unwrap().n_tx == 0 {
                return h;
            }
        }
        records
    }

    ///
    /// Get block header information.
    ///
    /// This is an in-memory query, thus very fast.
    /// This method is useful for computing blockchain statistics.
    ///
    /// # Example
    ///
    /// ## Compute total number of transactions
    /// ```rust
    /// use bitcoin_explorer::BitcoinDB;
    /// use std::path::Path;
    ///
    /// let path = Path::new("/Users/me/bitcoin");
    ///
    /// // launch without reading txindex
    /// let db = BitcoinDB::new(path, false).unwrap();
    ///
    /// let mut total_number_of_tx: usize = 0;
    ///
    /// // This computation should finish immediately. No Disk Access.
    /// for i in 0..db.get_block_count() {
    ///     let header = db.get_header(i).unwrap();
    ///     total_number_of_tx += header.n_tx as usize;
    /// }
    /// println!("total number of transactions found on disk : {}.", total_number_of_tx);
    /// ```
    ///
    pub fn get_header(&self, height: usize) -> Result<&BlockIndexRecord> {
        self.block_index
            .records
            .get(height)
            .ok_or(Error::BlockIndexNotFound(height))
    }

    /// Get block hash of a certain height.
    pub fn get_hash_from_height(&self, height: usize) -> Result<BlockHash> {
        self.block_index
            .records
            .get(height)
            .map(|s| s.block_header.block_hash())
            .ok_or(Error::BlockIndexNotFound(height))
    }

    /// Get block height of certain hash.
    ///
    /// Note that the hash is a hex string of the block hash.
    pub fn get_height_from_hash(&self, hash: &BlockHash) -> Result<usize> {
        self.block_index
            .hash_to_height
            .get(hash)
            .map(|h| *h as usize)
            .ok_or(Error::BlockHashNotFound(*hash))
    }

    /// Get a raw block as bytes
    pub fn get_raw_block(&self, height: usize) -> Result<Vec<u8>> {
        let index = self
            .block_index
            .records
            .get(height)
            .ok_or(Error::BlockIndexNotFound(height))?;
        let blk = self
            .blk_file
            .read_raw_block(index.n_file, index.n_data_pos)?;
        Ok(blk)
    }

    ///
    /// Get a block (in different formats (Block, FBlock, SBlock))
    ///
    /// # Example
    /// ```rust
    /// use bitcoin_explorer::{BitcoinDB, FBlock, SBlock, Block};
    /// use std::path::Path;
    ///
    /// let path = Path::new("/Users/me/bitcoin");
    ///
    /// // launch without reading txindex
    /// let db = BitcoinDB::new(path, false).unwrap();
    ///
    /// // get block of height 600000 (in different formats)
    /// let block: Block = db.get_block(600000).unwrap();
    /// let block: FBlock = db.get_block(600000).unwrap();
    /// let block: SBlock = db.get_block(600000).unwrap();
    /// ```
    ///
    pub fn get_block<T: From<Block>>(&self, height: usize) -> Result<T> {
        let index = self
            .block_index
            .records
            .get(height)
            .ok_or(Error::BlockIndexNotFound(height))?;
        let blk = self.blk_file.read_block(index.n_file, index.n_data_pos)?;
        Ok(blk.into())
    }

    ///
    /// Get a transaction by providing txid.
    ///
    /// This function requires `txindex` to be set to `true` for `BitcoinDB`,
    /// and requires that flag `txindex=1` has been enabled when
    /// running Bitcoin Core.
    ///
    /// A transaction cannot be found using this function if it is
    /// not yet indexed using `txindex`.
    ///
    /// # Example
    /// ```rust
    /// use bitcoin_explorer::{BitcoinDB, Transaction, FTransaction, STransaction, Txid, FromHex};
    /// use std::path::Path;
    ///
    /// let path = Path::new("/Users/me/bitcoin");
    ///
    /// // !!must launch with txindex=true!!
    /// let db = BitcoinDB::new(path, true).unwrap();
    ///
    /// // get transaction
    /// // e3bf3d07d4b0375638d5f1db5255fe07ba2c4cb067cd81b84ee974b6585fb468
    /// let txid_str = "e3bf3d07d4b0375638d5f1db5255fe07ba2c4cb067cd81b84ee974b6585fb468";
    /// let txid = Txid::from_hex(txid_str).unwrap();
    ///
    /// // get transactions in different formats
    /// let tx: Transaction = db.get_transaction(&txid).unwrap();
    /// let tx: FTransaction = db.get_transaction(&txid).unwrap();
    /// let tx: STransaction = db.get_transaction(&txid).unwrap();
    /// ```
    ///
    pub fn get_transaction<T: From<Transaction>>(&self, txid: &Txid) -> Result<T> {
        if !self.tx_db.is_open() {
            return Err(Error::TxDbUnavailable);
        }
        // give special treatment for genesis transaction
        if self.tx_db.is_genesis_tx(txid) {
            return Ok(self.get_block::<Block>(0)?.txdata.swap_remove(0).into());
        }
        let record = self.tx_db.get_tx_record(txid)?;
        let tx = self
            .blk_file
            .read_transaction(record.n_file, record.n_pos, record.n_tx_offset)?;
        Ok(tx.into())
    }

    ///
    /// Get the height of the block containing a particular transaction.
    ///
    /// This function requires `txindex` to be set to `true` for `BitcoinDB`,
    /// and requires that flag `txindex=1` has been enabled when
    /// running Bitcoin Core.
    ///
    /// A transaction cannot be found using this function if it is
    /// not yet indexed using `txindex`.
    ///
    pub fn get_height_of_transaction(&self, txid: &Txid) -> Result<usize> {
        if !self.tx_db.is_open() {
            return Err(Error::TxDbUnavailable);
        }
        self.tx_db.get_block_height_of_tx(txid)
    }

    ///
    /// Iterate through all blocks from `start` to `end` (excluded).
    ///
    /// Formats: `Block` / `FBlock` / `SBlock`.
    ///
    /// # Performance
    ///
    /// This iterator is implemented to read the blocks in concurrency,
    /// but the result is still produced in sequential order.
    /// Results read are stored in a synced queue for `next()`
    /// to get.
    ///
    /// The iterator stops automatically when a block cannot be
    /// read (i.e., when the max height in the database met).
    ///
    /// This is a very efficient implementation.
    /// Using SSD and intel core i7 (4 core, 8 threads)
    /// Iterating from height 0 to 700000 takes about 10 minutes.
    ///
    /// # Example
    ///
    /// ```rust
    /// use bitcoin_explorer::{BitcoinDB, Block, SBlock, FBlock};
    /// use std::path::Path;
    ///
    /// let path = Path::new("/Users/me/bitcoin");
    ///
    /// // launch without reading txindex
    /// let db = BitcoinDB::new(path, false).unwrap();
    ///
    /// // iterate over block from 600000 to 700000
    /// for block in db.iter_block::<Block>(600000, 700000) {
    ///     for tx in block.txdata {
    ///         println!("do something for this transaction");
    ///     }
    /// }
    ///
    /// // iterate over block from 600000 to 700000
    /// for block in db.iter_block::<FBlock>(600000, 700000) {
    ///     for tx in block.txdata {
    ///         println!("do something for this transaction");
    ///     }
    /// }
    ///
    /// // iterate over block from 600000 to 700000
    /// for block in db.iter_block::<SBlock>(600000, 700000) {
    ///     for tx in block.txdata {
    ///         println!("do something for this transaction");
    ///     }
    /// }
    /// ```
    ///
    pub fn iter_block<T>(&self, start: usize, end: usize) -> BlockIter<T>
    where
        T: From<Block> + Send + 'static,
    {
        BlockIter::from_range(self, start, end)
    }

    ///
    /// Iterate through all blocks of given heights.
    ///
    /// Formats: `Block` / `FBlock` / `SBlock`.
    ///
    /// # Performance
    ///
    /// This iterator is implemented to read the blocks in concurrency,
    /// but the result is still produced in the given order in `heights`.
    /// Results read are stored in a synced queue for `next()`
    /// to get.
    ///
    /// This is a very efficient implementation.
    /// Using SSD and intel core i7 (4 core, 8 threads)
    /// Iterating from height 0 to 700000 takes about 10 minutes.
    ///
    /// ## Fails Fast
    ///
    /// The iterator stops immediately when a `height` cannot be found.
    ///
    /// # Example
    ///
    /// ```rust
    /// use bitcoin_explorer::{BitcoinDB, Block, FBlock, SBlock};
    /// use std::path::Path;
    ///
    /// let path = Path::new("/Users/me/bitcoin");
    ///
    /// // launch without reading txindex
    /// let db = BitcoinDB::new(path, false).unwrap();
    ///
    /// let some_heights = vec![3, 5, 7, 9];
    ///
    /// // iterate over blocks from 600000 to 700000
    /// for block in db.iter_heights::<Block, _>(some_heights.clone()) {
    ///     for tx in block.txdata {
    ///         println!("do something for this transaction");
    ///     }
    /// }
    ///
    /// // iterate over simple blocks from 600000 to 700000
    /// for block in db.iter_heights::<SBlock, _>(some_heights.clone()) {
    ///     for tx in block.txdata {
    ///         println!("do something for this transaction");
    ///     }
    /// }
    ///
    /// // iterate over full blocks from 600000 to 700000
    /// for block in db.iter_heights::<FBlock, _>(some_heights.clone()) {
    ///     for tx in block.txdata {
    ///         println!("do something for this transaction");
    ///     }
    /// }
    ///
    ///
    /// ```
    ///
    pub fn iter_heights<T, TIter>(&self, heights: TIter) -> BlockIter<T>
    where
        T: 'static + From<Block> + Send,
        TIter: IntoIterator<Item = usize> + Send + 'static,
        <TIter as IntoIterator>::IntoIter: Send + 'static,
    {
        BlockIter::new(self, heights)
    }

    ///
    /// Get a block with inputs replaced by connected outputs.
    ///
    /// This function requires `txindex` to be set to `true` for `BitcoinDB`,
    /// and requires that flag `txindex=1` has been enabled when
    /// running Bitcoin Core.
    ///
    /// A transaction cannot be found using this function if it is
    /// not yet indexed using `txindex`.
    ///
    /// # Caveat!!
    ///
    /// ## Performance Warning
    ///
    /// Slow! For massive computation, use `db.iter_connected_block()`.
    ///
    pub fn get_connected_block<T: ConnectedBlock>(&self, height: usize) -> Result<T> {
        if !self.tx_db.is_open() {
            return Err(Error::TxDbUnavailable);
        }
        let tx = self.get_block(height)?;
        T::connect(tx, &self.tx_db, &self.block_index, &self.blk_file)
    }

    ///
    /// Get a transaction with outpoints replaced by outputs.
    ///
    /// This function requires `txindex` to be set to `true` for `BitcoinDB`,
    /// and requires that flag `txindex=1` has been enabled when
    /// running Bitcoin Core.
    ///
    /// A transaction cannot be found using this function if it is
    /// not yet indexed using `txindex`.
    ///
    /// Format: `full (FConnectedTransaction)` / `simple (SConnectedTransaction)`.
    ///
    /// # Caveats
    ///
    /// ## Performance Warning
    ///
    /// Slow! For massive computation, use `db.iter_connected_block()`.
    ///
    pub fn get_connected_transaction<T: ConnectedTx>(&self, txid: &Txid) -> Result<T> {
        if !self.tx_db.is_open() {
            return Err(Error::TxDbUnavailable);
        }
        let tx = self.get_transaction(txid)?;
        T::connect(tx, &self.tx_db, &self.block_index, &self.blk_file)
    }

    ///
    /// Iterate through all blocks for a given heights (excluded).
    ///
    /// Format: `full (FConnectedBlock)` / `simple (SConnectedBlock)`.
    ///
    /// This iterator use `unspent output` to track down the connected
    /// outputs of each outpoints.
    ///
    /// ## Note
    /// This does NOT require `txindex=true`.
    ///
    /// # Performance
    ///
    /// ## Using default feature:
    /// Requires 4 GB memory, finishes in 2.5 hours from 0-70000 block.
    ///
    /// ## Using non-default feature
    /// Requires 32 GB memory, finished in 30 minutes from 0-70000 block.
    ///
    /// # Example
    ///
    /// ```rust
    /// use bitcoin_explorer::{BitcoinDB, FConnectedBlock, SConnectedBlock};
    /// use std::path::Path;
    ///
    /// let path = Path::new("/Users/me/bitcoin");
    ///
    /// // launch without reading txindex
    /// let db = BitcoinDB::new(path, false).unwrap();
    ///
    /// // iterate over block from 0 to 700000, (simple format)
    /// for block in db.iter_connected_block::<SConnectedBlock>(700000) {
    ///     for tx in block.txdata {
    ///         println!("do something for this transaction");
    ///     }
    /// }
    /// ```
    ///
    pub fn iter_connected_block<TBlock>(&self, end: usize) -> ConnectedBlockIter<TBlock>
    where
        TBlock: 'static + ConnectedBlock + Send,
    {
        ConnectedBlockIter::new(self, end)
    }
}
