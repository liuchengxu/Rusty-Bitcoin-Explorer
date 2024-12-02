//! This module defines various formats of blockchain data representation.
//!
//! ## Basic Block Types
//!
//! There are three variants of basic block types.
//! - Block: imported from rust-bitcoin
//! - FullBlock: `full_block::FullBlock`, with extra info pre-computed.
//! - CompactBlock: `compact_block::CompactBlock`, with minimal amount of necessary info.
//!
//! For details, see the struct documentations.
//!
//! ## Connected Blocks
//!
//! Connected blocks are blocks with input replaced by referred outputs.
//! There are two types:
//! - `CompactConnectedBlock`
//! - `FullConnectedBlock`
//!     Corresponding to the basic F/S Blocks.

pub mod compact_block;
pub mod connected_block;
pub mod full_block;
