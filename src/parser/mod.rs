//! This module defines how to parse binary data on disk to Block structs defined in proto.

pub mod blk_file;
pub mod block_index;
pub mod error;
pub mod proto;
pub mod reader;
pub mod script;
pub mod tx_index;
pub(crate) mod xor;
