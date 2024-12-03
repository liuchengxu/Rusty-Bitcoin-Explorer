//! connect outpoints of inputs to previous outputs

use super::compact_block::{CompactBlockHeader, CompactTxOut};
use super::full_block::{FullBlockHeader, FullTxOut};
use crate::parser::blk_file::BlkFile;
use crate::parser::error::{Error, Result};
use crate::parser::tx_index::TxDB;
use crate::{BlockHeader, BlockIndex};
use bitcoin::{Block, BlockHash, Transaction, TxIn, TxOut, Txid};
use log::warn;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// This type refer to `Block` structs where inputs are
/// replaced by connected outputs.
///
/// ## Implementors:
/// - CompactConnectedBlock
/// - FullConnectedBlock
pub trait ConnectedBlock {
    /// Associated output type.
    type Tx: ConnectedTx + Send;

    /// Construct a ConnectedBlock from parts of a block.
    ///
    /// Used in `connected_block_iter.rs`.
    fn from(block_header: BlockHeader, block_hash: BlockHash) -> Self;

    /// Add a new transaction in this block.
    ///
    /// Used in `connected_block_iter.rs`.
    fn add_tx(&mut self, tx: Self::Tx);

    /// Construct a ConnectedBlock and connect the transactions.
    fn connect(
        block: Block,
        tx_db: &TxDB,
        blk_index: &BlockIndex,
        blk_file: &BlkFile,
    ) -> Result<Self>
    where
        Self: Sized;
}

/// This type refer to `Transaction` structs where inputs are
/// replaced by connected outputs.
///
/// ## Implementors:
/// - CompactTransaction
/// - FullTransaction
pub trait ConnectedTx {
    /// Associated output type.
    type TxOut: 'static + From<TxOut> + Send;

    /// Construct a ConnectedTx from Transaction without blank inputs.
    ///
    /// This function is used in `connected_block_iter.rs`.
    fn from(tx: &Transaction) -> Self;

    /// Add a input to this ConnectedTx.
    ///
    /// This function is used in `connected_block_iter.rs`.
    fn add_input(&mut self, input: Self::TxOut);

    /// Build ConnectedTx from Tx,
    /// and attach inputs to this ConnectedTx using tx-index.
    fn connect(
        tx: Transaction,
        tx_db: &TxDB,
        blk_index: &BlockIndex,
        blk_file: &BlkFile,
    ) -> Result<Self>
    where
        Self: Sized;
}

/// Simple format of connected block.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct CompactConnectedBlock {
    pub header: CompactBlockHeader,
    pub txdata: Vec<CompactConnectedTransaction>,
}

/// Full format of connected block.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct FullConnectedBlock {
    pub header: FullBlockHeader,
    pub txdata: Vec<FullConnectedTransaction>,
}

/// Simple format of connected transaction.
/// See fields for details of this struct.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct CompactConnectedTransaction {
    pub txid: Txid,
    pub input: Vec<CompactTxOut>,
    pub output: Vec<CompactTxOut>,
}

/// Full format of connected transaction.
/// See fields for details of this struct.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct FullConnectedTransaction {
    pub version: i32,
    pub lock_time: u32,
    pub txid: Txid,
    pub input: Vec<FullTxOut>,
    pub output: Vec<FullTxOut>,
}

impl ConnectedTx for FullConnectedTransaction {
    type TxOut = FullTxOut;

    fn from(tx: &Transaction) -> Self {
        Self {
            version: tx.version.0,
            lock_time: tx.lock_time.to_consensus_u32(),
            txid: tx.compute_txid(),
            input: Vec::new(),
            output: tx.output.clone().into_iter().map(|x| x.into()).collect(),
        }
    }

    fn add_input(&mut self, input: Self::TxOut) {
        self.input.push(input);
    }

    fn connect(
        tx: Transaction,
        tx_db: &TxDB,
        blk_index: &BlockIndex,
        blk_file: &BlkFile,
    ) -> Result<Self> {
        let is_coinbase = tx.is_coinbase();
        Ok(Self {
            version: tx.version.0,
            lock_time: tx.lock_time.to_consensus_u32(),
            txid: tx.compute_txid(),
            input: connect_tx_inputs(&tx.input, is_coinbase, tx_db, blk_index, blk_file)?
                .into_iter()
                .map(Into::into)
                .collect(),
            output: tx.output.into_iter().map(Into::into).collect(),
        })
    }
}

impl ConnectedTx for CompactConnectedTransaction {
    type TxOut = CompactTxOut;

    fn from(tx: &Transaction) -> Self {
        Self {
            txid: tx.compute_txid(),
            input: Vec::new(),
            output: tx.output.clone().into_iter().map(Into::into).collect(),
        }
    }

    fn add_input(&mut self, input: Self::TxOut) {
        self.input.push(input);
    }

    fn connect(
        tx: Transaction,
        tx_db: &TxDB,
        blk_index: &BlockIndex,
        blk_file: &BlkFile,
    ) -> Result<Self> {
        let is_coinbase = tx.is_coinbase();
        Ok(Self {
            txid: tx.compute_txid(),
            input: connect_tx_inputs(&tx.input, is_coinbase, tx_db, blk_index, blk_file)?
                .into_iter()
                .map(Into::into)
                .collect(),
            output: tx.output.into_iter().map(Into::into).collect(),
        })
    }
}

impl ConnectedBlock for FullConnectedBlock {
    type Tx = FullConnectedTransaction;

    fn from(block_header: BlockHeader, block_hash: BlockHash) -> Self {
        Self {
            header: FullBlockHeader::parse(block_header, block_hash),
            txdata: Vec::new(),
        }
    }

    fn add_tx(&mut self, tx: Self::Tx) {
        self.txdata.push(tx);
    }

    fn connect(
        block: Block,
        tx_db: &TxDB,
        blk_index: &BlockIndex,
        blk_file: &BlkFile,
    ) -> Result<Self> {
        let block_hash = block.header.block_hash();
        Ok(Self {
            header: FullBlockHeader::parse(block.header, block_hash),
            txdata: connect_block_inputs(block.txdata, tx_db, blk_index, blk_file)?,
        })
    }
}

impl ConnectedBlock for CompactConnectedBlock {
    type Tx = CompactConnectedTransaction;

    fn from(block_header: BlockHeader, block_hash: BlockHash) -> Self {
        Self {
            header: CompactBlockHeader::new(block_header, block_hash),
            txdata: Vec::new(),
        }
    }

    fn add_tx(&mut self, tx: Self::Tx) {
        self.txdata.push(tx);
    }

    fn connect(
        block: Block,
        tx_db: &TxDB,
        blk_index: &BlockIndex,
        blk_file: &BlkFile,
    ) -> Result<Self> {
        let block_hash = block.header.block_hash();
        Ok(Self {
            header: CompactBlockHeader::new(block.header, block_hash),
            txdata: connect_block_inputs(block.txdata, tx_db, blk_index, blk_file)?,
        })
    }
}

/// This function is used for connecting transaction inputs for a single block.
#[inline]
fn connect_block_inputs<Tx>(
    transactions: Vec<Transaction>,
    tx_db: &TxDB,
    blk_index: &BlockIndex,
    blk_file: &BlkFile,
) -> Result<Vec<Tx>>
where
    Tx: ConnectedTx,
{
    // Collect all transaction inputs from the block's transactions.
    let all_tx_in: Vec<_> = transactions.iter().flat_map(|tx| tx.input.iter()).collect();

    // connect transactions inputs in parallel
    let mut connected_outputs: VecDeque<Option<TxOut>> = all_tx_in
        .par_iter()
        .map(|x| connect_input(x, tx_db, blk_index, blk_file))
        .collect();

    // reconstruct block
    let mut connected_tx = Vec::with_capacity(transactions.len());

    for tx in transactions {
        let outpoints_count = if tx.is_coinbase() { 0 } else { tx.input.len() };

        let mut outputs = Vec::with_capacity(outpoints_count);

        for _ in &tx.input {
            let connected_out = connected_outputs.pop_front().unwrap();
            if let Some(out) = connected_out {
                // also do not push the null input connected to coinbase transaction
                outputs.push(out);
            }
        }

        // check if any output is missing
        if outputs.len() != outpoints_count {
            return Err(Error::MissingOutputs {
                expected: outpoints_count,
                got: outputs.len(),
            });
        }

        let mut tx = Tx::from(&tx);
        for output in outputs {
            tx.add_input(output.into());
        }

        connected_tx.push(tx);
    }

    Ok(connected_tx)
}

/// This function converts multiple Inputs of a single transaction to Outputs in parallel.
#[inline]
fn connect_tx_inputs(
    tx_in: &[TxIn],
    is_coinbase: bool,
    tx_db: &TxDB,
    blk_index: &BlockIndex,
    blk_file: &BlkFile,
) -> Result<Vec<TxOut>> {
    let connected_outputs: Vec<TxOut> = tx_in
        .par_iter()
        .filter_map(|x| connect_input(x, tx_db, blk_index, blk_file))
        .collect();

    let outpoints_count = if is_coinbase { 0 } else { tx_in.len() };
    let received = connected_outputs.len();

    // some outpoints aren't found
    if received != outpoints_count {
        Err(Error::MissingOutputs {
            expected: outpoints_count,
            got: received,
        })
    } else {
        Ok(connected_outputs)
    }
}

/// This function connect a single TxIn to outputs. It converts:
/// - read failure to `None`
/// - coinbase transaction output to `None`
#[inline]
fn connect_input(
    tx_in: &TxIn,
    tx_db: &TxDB,
    blk_index: &BlockIndex,
    blk_file: &BlkFile,
) -> Option<TxOut> {
    // skip coinbase transaction
    if is_coin_base(tx_in) {
        return None;
    }

    let outpoint = tx_in.previous_output;
    let tx_id = outpoint.txid;

    // special treatment of genesis tx, which cannot be found in tx-index.
    if tx_db.is_genesis_tx(tx_id) {
        let pos = blk_index.records.first()?;
        return match blk_file.read_block(pos.n_file, pos.n_data_pos) {
            Ok(mut blk) => {
                let mut tx = blk.txdata.swap_remove(0);
                Some(tx.output.swap_remove(0))
            }
            Err(_) => None,
        };
    }

    let n = outpoint.vout;

    if let Ok(record) = tx_db.get_tx_record(tx_id) {
        if let Ok(mut tx) =
            blk_file.read_transaction(record.n_file, record.n_pos, record.n_tx_offset)
        {
            let len = tx.output.len();
            if n >= len as u32 {
                warn!("outpoint {outpoint} exceeds range");
                None
            } else {
                Some(tx.output.swap_remove(n as usize))
            }
        } else {
            warn!("fail to read transaction for {outpoint}");
            None
        }
    } else {
        warn!("cannot find outpoint {outpoint} in txDB");
        None
    }
}

#[inline]
fn is_coin_base(tx_in: &TxIn) -> bool {
    tx_in.previous_output.is_null()
}
