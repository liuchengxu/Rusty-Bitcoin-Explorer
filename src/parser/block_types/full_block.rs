//! Full Block Representation with Additional Metadata
//!
//! This module provides extended information for Bitcoin blocks and transactions,
//! adding precomputed attributes such as block hash, transaction ID, output addresses,
//! and output script types to the original [`bitcoin::Block`] structure.
//!
//! Key differences from the base [`bitcoin::Block`]:
//! - `FullBlock` includes computed block hash, transaction IDs, output addresses, and script types.
//! - `FullTransaction` contains precomputed transaction ID, output addresses, and script types.
//! - `FullTxOut` includes precomputed script types and addresses for each output.

use crate::api::Block;
use crate::parser::script::{evaluate_script, ScriptType};
use bitcoin::hash_types::TxMerkleNode;
use bitcoin::{Address, BlockHash, Transaction, TxOut, Txid};
use serde::{Deserialize, Serialize};

/// A Bitcoin block with additional metadata.
///
/// A [`FullBlock`] extends the [`bitcoin::Block`] by adding precomputed attributes:
/// - `block_hash`: The hash of the block.
/// - `txid`: Transaction IDs for each transaction in the block.
/// - `output_addresses`: List of addresses for each transaction output.
/// - `output_script_types`: Script types for each transaction output.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct FullBlock {
    pub header: FullBlockHeader,
    pub txdata: Vec<FullTransaction>,
}

impl From<Block> for FullBlock {
    /// Converts a `bitcoin::Block` to a `FullBlock` by computing additional metadata.
    fn from(block: bitcoin::Block) -> Self {
        let block_hash = block.header.block_hash();
        Self {
            header: FullBlockHeader::parse(block.header, block_hash),
            txdata: block.txdata.into_iter().map(|x| x.into()).collect(),
        }
    }
}

/// Full header of a Bitcoin block, with added `block_hash` compared to the base [`crate::BlockHeader`].
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct FullBlockHeader {
    pub version: i32,
    pub prev_blockhash: BlockHash,
    pub merkle_root: TxMerkleNode,
    pub time: u32,
    pub bits: u32,
    pub nonce: u32,
    /// Precomputed.
    pub block_hash: BlockHash,
}

impl FullBlockHeader {
    /// Creates a `FullBlockHeader` from a base `BlockHeader` and the computed `block_hash`.
    pub fn parse(b: crate::BlockHeader, block_hash: BlockHash) -> Self {
        Self {
            version: b.version.to_consensus(),
            block_hash,
            prev_blockhash: b.prev_blockhash,
            merkle_root: b.merkle_root,
            time: b.time,
            bits: b.bits.to_consensus(),
            nonce: b.nonce,
        }
    }
}

/// A Bitcoin transaction with additional metadata.
///
/// A [`FullTransaction`] extends the [`Transaction`] by adding precomputed metadata:
/// - `txid`: The transaction ID.
/// - `output_addresses`: The list of addresses for each transaction output.
/// - `output_script_types`: The script type for each transaction output.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct FullTransaction {
    pub version: i32,
    pub lock_time: u32,
    pub input: Vec<bitcoin::TxIn>,
    /// Precomputed transaction ID.
    pub txid: Txid,
    /// List of outputs, with additional metadata.
    pub output: Vec<FullTxOut>,
}

impl From<Transaction> for FullTransaction {
    fn from(tx: Transaction) -> Self {
        let is_coinbase = tx.is_coinbase();
        let txid = tx.compute_txid();
        let input = if is_coinbase { Vec::new() } else { tx.input };
        Self {
            version: tx.version.0,
            lock_time: tx.lock_time.to_consensus_u32(),
            txid,
            input,
            output: tx.output.into_iter().map(FullTxOut::from).collect(),
        }
    }
}

/// A Bitcoin transaction output with additional metadata.
///
/// A [`FullTxOut`] extends the [`bitcoin::TxOut`] by adding precomputed information:
/// - `script_type`: The type of the output's script.
/// - `addresses`: The list of addresses associated with the output.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct FullTxOut {
    pub value: u64,
    pub script_pubkey: bitcoin::ScriptBuf,
    /// Precomputed scrip type.
    pub script_type: ScriptType,
    /// Precomputed addresses associated with the output.
    pub addresses: Box<[Address]>,
}

impl From<TxOut> for FullTxOut {
    fn from(out: bitcoin::TxOut) -> Self {
        let eval = evaluate_script(&out.script_pubkey, bitcoin::Network::Bitcoin);
        Self {
            value: out.value.to_sat(),
            script_pubkey: out.script_pubkey,
            script_type: eval.pattern,
            addresses: eval.addresses.into_boxed_slice(),
        }
    }
}
