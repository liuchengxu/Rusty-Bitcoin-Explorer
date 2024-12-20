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
pub use crate::parser::block_types::compact_block::{
    CompactBlock, CompactBlockHeader, CompactTransaction, CompactTxOut,
};
pub use crate::parser::block_types::connected_block::{
    CompactConnectedBlock, CompactConnectedTransaction, ConnectedBlock, ConnectedTx,
    FullConnectedBlock, FullConnectedTransaction,
};
pub use crate::parser::block_types::full_block::{
    FullBlock, FullBlockHeader, FullTransaction, FullTxOut,
};
pub use bitcoin::blockdata::block::Header as BlockHeader;
pub use bitcoin::hashes::hex::FromHex;
pub use bitcoin::{Address, Block, BlockHash, Network, Script, ScriptBuf, Transaction, Txid};

/// Extract addresses from a script public key.
#[deprecated(since = "1.2.7", note = "use `get_addresses_from_script` instead")]
pub fn parse_script(script_pub_key: &str) -> Result<ScriptInfo> {
    get_addresses_from_script(script_pub_key)
}

/// Extract addresses from a script public key.
#[inline]
pub fn get_addresses_from_script(script_pub_key: &str) -> Result<ScriptInfo> {
    let script_buf = ScriptBuf::from_hex(script_pub_key)?;
    Ok(evaluate_script(script_buf.as_script(), Network::Bitcoin))
}

pub struct InnerDB {
    pub blk_file: BlkFile,
    pub block_index: BlockIndex,
    pub tx_db: Option<TxDB>,
}

/// This is the main struct of this crate!! Click and read the doc.
///
/// All queries start from initializing `BitcoinDB`.
///
/// Note: This is an Arc wrap around `InnerDB`.
#[derive(Clone)]
pub struct BitcoinDB(Arc<InnerDB>);

impl Deref for BitcoinDB {
    type Target = InnerDB;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl BitcoinDB {
    /// Represents a Bitcoin blockchain data reader.
    ///
    /// # Arguments
    ///
    /// `data_dir`: The directory containing Bitcoin blockchain data (specified by `-datadir` in Bitcoin Core).
    /// `tx_index`: Flag indicating whether to attempt to open the transaction index (txindex) levelDB.
    ///
    /// # Example
    ///
    /// ```rust
    /// use bitcoin_explorer::BitcoinDB;
    /// use std::path::Path;
    ///
    /// let path = Path::new("/Users/me/bitcoin");
    ///
    /// // Launch without reading txindex
    /// let db = BitcoinDB::new(path, false).unwrap();
    ///
    /// // Launch attempting to read txindex
    /// let db = BitcoinDB::new(path, true).unwrap();
    /// ```
    pub fn new(data_dir: &Path, tx_index: bool) -> Result<Self> {
        if !data_dir.exists() {
            return Err(Error::BitcoinDataDirDoesNotExist(data_dir.to_path_buf()));
        }

        let blk_path = data_dir.join("blocks");
        let block_index = BlockIndex::new(blk_path.join("index"))?;

        let tx_db = if tx_index {
            let tx_index_path = data_dir.join("indexes").join("txindex");
            TxDB::open(&tx_index_path, &block_index)
        } else {
            None
        };

        let inner = InnerDB {
            block_index,
            blk_file: BlkFile::new(blk_path.as_path())?,
            tx_db,
        };

        Ok(Self(Arc::new(inner)))
    }

    /// Get the maximum height found in block index.
    ///
    /// Note, not all blocks lower than this height have
    /// been downloaded (different from `get_block_count()`).
    ///
    /// Deprecated: use `get_block_count()`
    #[deprecated(since = "1.2.6", note = "use `get_block_count()` instead")]
    pub fn get_max_height(&self) -> usize {
        self.block_index.records.len()
    }

    /// Get the maximum number of blocks downloaded.
    ///
    /// This API guarantee that block 0 to `get_block_count() - 1`
    /// have been downloaded and available for query.
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

    /// Get block header information.
    ///
    /// This is an in-memory query, so it's very fast and doesn't involve disk access.
    /// It is useful for computing blockchain statistics such as the total number of transactions.
    ///
    /// # Example
    ///
    /// ## Compute total number of transactions
    ///
    /// ```rust
    /// use bitcoin_explorer::BitcoinDB;
    /// use std::path::Path;
    ///
    /// let path = Path::new("/Users/me/bitcoin");
    ///
    /// // Launch without reading txindex
    /// let db = BitcoinDB::new(path, false).unwrap();
    ///
    /// let mut total_number_of_tx: usize = 0;
    ///
    /// // This computation should finish immediately. No Disk Access.
    /// for i in 0..db.get_block_count() {
    ///     let header = db.get_header(i).unwrap();
    ///     total_number_of_tx += header.n_tx as usize;
    /// }
    /// println!("Total number of transactions found on disk: {}", total_number_of_tx);
    /// ```
    pub fn get_header(&self, height: usize) -> Result<&BlockIndexRecord> {
        self.block_index
            .records
            .get(height)
            .ok_or(Error::BlockIndexRecordNotFound(height))
    }

    /// Get block hash of a certain height.
    pub fn get_hash_from_height(&self, height: usize) -> Result<BlockHash> {
        self.block_index
            .records
            .get(height)
            .map(|s| s.block_header.block_hash())
            .ok_or(Error::BlockIndexRecordNotFound(height))
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
            .ok_or(Error::BlockIndexRecordNotFound(height))?;
        self.blk_file.read_raw_block(index.n_file, index.n_data_pos)
    }

    /// Get a block (in different formats (Block, FullBlock, CompactBlock))
    ///
    /// # Example
    /// ```rust
    /// use bitcoin_explorer::{BitcoinDB, FullBlock, CompactBlock, Block};
    /// use std::path::Path;
    ///
    /// let path = Path::new("/Users/me/bitcoin");
    ///
    /// // launch without reading txindex
    /// let db = BitcoinDB::new(path, false).unwrap();
    ///
    /// // get block of height 600000 (in different formats)
    /// let block: Block = db.get_block(600000).unwrap();
    /// let block: FullBlock = db.get_block(600000).unwrap();
    /// let block: CompactBlock = db.get_block(600000).unwrap();
    /// ```
    pub fn get_block<T: From<Block>>(&self, height: usize) -> Result<T> {
        let index = self
            .block_index
            .records
            .get(height)
            .ok_or(Error::BlockIndexRecordNotFound(height))?;
        self.blk_file
            .read_block(index.n_file, index.n_data_pos)
            .map(Into::into)
    }

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
    /// use bitcoin_explorer::{BitcoinDB, Transaction, FullTransaction, CompactTransaction, Txid, FromHex};
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
    /// let tx: Transaction = db.get_transaction(txid).unwrap();
    /// let tx: FullTransaction = db.get_transaction(txid).unwrap();
    /// let tx: CompactTransaction = db.get_transaction(txid).unwrap();
    /// ```
    pub fn get_transaction<T: From<Transaction>>(&self, txid: Txid) -> Result<T> {
        let tx_db = self.tx_db.as_ref().ok_or(Error::TxDbUnavailable)?;

        // give special treatment for genesis transaction
        if tx_db.is_genesis_tx(txid) {
            return Ok(self.get_block::<Block>(0)?.txdata.swap_remove(0).into());
        }

        let tx_pos = tx_db.get_tx_position(txid)?;
        self.blk_file
            .read_transaction(tx_pos.n_file, tx_pos.n_data_pos, tx_pos.n_tx_offset)
            .map(Into::into)
    }

    /// Returns the height of the block containing the given transaction ID.
    ///
    /// This function requires `txindex` to be set to `true` for `BitcoinDB`,
    /// and requires that flag `txindex=1` has been enabled when
    /// running Bitcoin Core.
    ///
    /// A transaction cannot be found using this function if it is
    /// not yet indexed using `txindex`.
    pub fn get_block_height(&self, txid: Txid) -> Result<usize> {
        let tx_db = self.tx_db.as_ref().ok_or(Error::TxDbUnavailable)?;
        tx_db.get_block_height(txid)
    }

    /// Iterate through all blocks from `start` to `end` (excluded).
    ///
    /// Formats: `Block` / `FullBlock` / `CompactBlock`.
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
    /// use bitcoin_explorer::{BitcoinDB, Block, CompactBlock, FullBlock};
    /// use std::path::Path;
    ///
    /// let path = Path::new("/Users/me/bitcoin");
    ///
    /// // launch without reading txindex
    /// let db = BitcoinDB::new(path, false).unwrap();
    ///
    /// // iterate over block from 600000 to 700000
    /// for block in db.block_iter::<Block>(600000, 700000) {
    ///     for tx in block.txdata {
    ///         println!("do something for this transaction");
    ///     }
    /// }
    ///
    /// // iterate over block from 600000 to 700000
    /// for block in db.block_iter::<FullBlock>(600000, 700000) {
    ///     for tx in block.txdata {
    ///         println!("do something for this transaction");
    ///     }
    /// }
    ///
    /// // iterate over block from 600000 to 700000
    /// for block in db.block_iter::<CompactBlock>(600000, 700000) {
    ///     for tx in block.txdata {
    ///         println!("do something for this transaction");
    ///     }
    /// }
    /// ```
    pub fn block_iter<B>(&self, start: usize, end: usize) -> BlockIter<B>
    where
        B: From<Block> + Send + 'static,
    {
        BlockIter::from_range(self, start, end)
    }

    /// Iterate through all blocks of given list of heights.
    ///
    /// Formats: `Block` / `FullBlock` / `CompactBlock`.
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
    /// use bitcoin_explorer::{BitcoinDB, Block, FullBlock, CompactBlock};
    /// use std::path::Path;
    ///
    /// let path = Path::new("/Users/me/bitcoin");
    ///
    /// // launch without reading txindex
    /// let db = BitcoinDB::new(path, false).unwrap();
    ///
    /// let some_heights = vec![3, 5, 7, 9];
    ///
    /// // iterate over blocks in the list [3, 5, 7, 9].
    /// for block in db.iter_heights::<Block, _>(some_heights.clone()) {
    ///     for tx in block.txdata {
    ///         println!("do something for this transaction");
    ///     }
    /// }
    /// ```
    pub fn iter_heights<B, I>(&self, heights: I) -> BlockIter<B>
    where
        B: 'static + From<Block> + Send,
        I: IntoIterator<Item = usize> + Send + 'static,
        <I as IntoIterator>::IntoIter: Send + 'static,
    {
        BlockIter::new(self, heights)
    }

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
    /// Slow! For massive computation, use `db.connected_block_iter()`.
    pub fn get_connected_block<T: ConnectedBlock>(&self, height: usize) -> Result<T> {
        let tx_db = self.tx_db.as_ref().ok_or(Error::TxDbUnavailable)?;
        let tx = self.get_block(height)?;
        T::connect(tx, tx_db, &self.block_index, &self.blk_file)
    }

    /// Get a transaction with outpoints replaced by outputs.
    ///
    /// This function requires `txindex` to be set to `true` for `BitcoinDB`,
    /// and requires that flag `txindex=1` has been enabled when
    /// running Bitcoin Core.
    ///
    /// A transaction cannot be found using this function if it is
    /// not yet indexed using `txindex`.
    ///
    /// Format: `full (FullConnectedTransaction)` / `simple (CompactConnectedTransaction)`.
    ///
    /// # Caveats
    ///
    /// ## Performance Warning
    ///
    /// Slow! For massive computation, use `db.connected_block_iter()`.
    pub fn get_connected_transaction<T: ConnectedTx>(&self, txid: Txid) -> Result<T> {
        let tx_db = self.tx_db.as_ref().ok_or(Error::TxDbUnavailable)?;
        let tx = self.get_transaction(txid)?;
        T::connect(tx, tx_db, &self.block_index, &self.blk_file)
    }

    /// Returns [`ConnectedBlockIter`] for iterating through all blocks for a given heights (excluded).
    ///
    /// Format: `full (FullConnectedBlock)` / `simple (CompactConnectedBlock)`.
    ///
    /// This iterator use `unspent output` to track down the connected
    /// outputs of each outpoints.
    ///
    /// ## Note
    ///
    /// This does NOT require `txindex=true`.
    ///
    /// # Performance
    ///
    /// ## Using default feature:
    ///
    /// Requires 4 GB memory, finishes in 2.5 hours from 0-700000 block.
    ///
    /// ## Using non-default feature
    ///
    /// Requires 32 GB memory, finished in 30 minutes from 0-700000 block.
    ///
    /// # Example
    ///
    /// ```rust
    /// use bitcoin_explorer::{BitcoinDB, FullConnectedBlock, CompactConnectedBlock};
    /// use std::path::Path;
    ///
    /// let path = Path::new("/Users/me/bitcoin");
    ///
    /// // launch without reading txindex
    /// let db = BitcoinDB::new(path, false).unwrap();
    ///
    /// // iterate over block from 0 to 700000, (simple format)
    /// for block in db.connected_block_iter::<CompactConnectedBlock>(700000) {
    ///     for tx in block.txdata {
    ///         println!("do something for this transaction");
    ///     }
    /// }
    /// ```
    pub fn connected_block_iter<B>(&self, end: usize) -> ConnectedBlockIter<B>
    where
        B: ConnectedBlock + Send + 'static,
    {
        ConnectedBlockIter::new(self, end)
    }
}
