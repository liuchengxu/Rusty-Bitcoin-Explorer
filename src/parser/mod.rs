//! This module defines how to parse binary data on disk to Block structs.

pub mod blk_file;
pub mod block_index;
pub mod block_types;
pub mod error;
pub mod reader;
pub mod script;
pub mod tx_index;
pub(crate) mod xor;
