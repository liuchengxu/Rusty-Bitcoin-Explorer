//! Simplified Blockchain Block Representation for Faster Processing (e.g., Python)
//!
//! This module defines compact version of Bitcoin block. These objects
//! are optimized to store only essential data, reducing the size of serialized data
//! for faster processing, especially in environments such as Python or other systems
//! that require a lighter data format.
//!
//! Key differences from the full Bitcoin data structures:
//! - `CompactBlock` and `CompactTransaction` contain only essential fields,
//!   omitting less critical data like previous block hash, Merkle root, and
//!   input witness for efficient processing.

use crate::parser::script::evaluate_script;
use bitcoin::{Address, Block, BlockHash, Transaction, TxIn, TxOut, Txid};
use serde::{Deserialize, Serialize};

/// A compact Bitcoin block containing only essential information for faster processing.
///
/// The `CompactBlock` includes the following attributes:
/// - `block_hash`: The hash of the block.
/// - `time`: The block's timestamp.
/// - `txid`: The transaction IDs of all transactions in the block.
/// - `output_addresses`: Addresses associated with the outputs.
/// - `output_script_types`: Script types for the transaction outputs.
///
/// It omits the following fields for reduced memory and transfer requirements:
/// - `nonce`
/// - `previous block hash`
/// - `merkle root`
/// - `bits`
/// - `input witness`
/// - `output public script key hash`
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct CompactBlock {
    pub header: CompactBlockHeader,
    pub txdata: Vec<CompactTransaction>,
}

impl From<Block> for CompactBlock {
    /// Add addresses, block_hash, tx_id to the bitcoin library format,
    /// and also simplify the format.
    fn from(block: Block) -> Self {
        let block_hash = block.header.block_hash();
        Self {
            header: CompactBlockHeader::new(block.header, block_hash),
            txdata: block.txdata.into_iter().map(|x| x.into()).collect(),
        }
    }
}

/// Simplified header of a Bitcoin block, including only essential fields.
///
/// A `CompactBlockHeader` includes:
/// - `block_hash`: The hash of the block.
/// - `time`: The timestamp of the block.
///
/// It omits the following fields:
/// - `nonce`
/// - `previous block hash`
/// - `merkle root`
/// - `bits`
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct CompactBlockHeader {
    pub block_hash: BlockHash,
    pub time: u32,
}

impl CompactBlockHeader {
    pub fn new(blk: crate::BlockHeader, block_hash: BlockHash) -> Self {
        Self {
            block_hash,
            time: blk.time,
        }
    }
}

/// A simplified Bitcoin transaction, including only essential information.
///
/// A `CompactTransaction` includes the following fields:
/// - `txid`: The transaction ID.
/// - `input`: A list of inputs (without the witness data).
/// - `output`: A list of outputs (with precomputed addresses and script types).
///
/// It omits the following fields for reduced memory and transfer requirements to Python:
/// - `input witness`
/// - `output public script key hash`
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct CompactTransaction {
    pub txid: Txid,
    /// List of inputs
    pub input: Vec<CompactTxIn>,
    /// List of outputs
    pub output: Vec<CompactTxOut>,
}

impl From<Transaction> for CompactTransaction {
    fn from(tx: Transaction) -> Self {
        let is_coinbase = tx.is_coinbase();
        let txid = tx.compute_txid();
        let input = if is_coinbase {
            Vec::new()
        } else {
            tx.input.into_iter().map(|x| x.into()).collect()
        };
        Self {
            txid,
            input,
            output: tx.output.into_iter().map(|x| x.into()).collect(),
        }
    }
}

/// A simplified Bitcoin transaction input, excluding the witness data.
///
/// A `CompactTxIn` includes:
/// - `txid`: The ID of the previous transaction.
/// - `vout`: The output index of the referenced transaction.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct CompactTxIn {
    pub txid: Txid,
    pub vout: u32,
}

impl From<TxIn> for CompactTxIn {
    fn from(tx_in: TxIn) -> Self {
        Self {
            txid: tx_in.previous_output.txid,
            vout: tx_in.previous_output.vout,
        }
    }
}

/// A simplified Bitcoin transaction output with precomputed addresses and script types.
///
/// A `CompactTxOut` includes:
/// - `value`: The value of the output.
/// - `addresses`: A list of addresses associated with the output.
///
/// It omits the following field:
/// - `output public script key hash`
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct CompactTxOut {
    pub value: u64,
    pub addresses: Box<[Address]>,
}

impl From<TxOut> for CompactTxOut {
    fn from(out: TxOut) -> Self {
        let eval = evaluate_script(&out.script_pubkey, bitcoin::Network::Bitcoin);
        Self {
            value: out.value.to_sat(),
            addresses: eval.addresses.into_boxed_slice(),
        }
    }
}
