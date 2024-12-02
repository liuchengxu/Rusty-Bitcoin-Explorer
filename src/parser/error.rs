use bitcoin::Txid;
use std::convert::{self, From};
use std::{string, sync};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("data_dir {0} does not exist")]
    BitcoinDataDirDoesNotExist(String),
    #[error("No blk files found!")]
    EmptyBlockFiles,
    #[error("blk file {0} not found, try to sync with Bitcoin Core")]
    BlockFileNotFound(i32),
    #[error("block index record {0} not found")]
    BlockIndexRecordNotFound(usize),
    #[error("block index for {0} not found")]
    BlockHashNotFound(bitcoin::BlockHash),
    #[error("Transaction record not found for {0}")]
    TransactionRecordNotFound(Txid),
    #[error("Some outpoints are not found, tx_index is not fully synced")]
    MissingOutputs { expected: usize, got: usize },
    #[error("failed to find height for transaction: {0}")]
    CannotFindHeightForTransaction(Txid),
    #[error("TxDB not open")]
    TxDbUnavailable,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    BitcoinIo(#[from] bitcoin::io::Error),
    #[error(transparent)]
    Encode(#[from] bitcoin::consensus::encode::Error),
    #[error("Invalid hash: {0}")]
    InvalidHash(String),
    #[error(transparent)]
    Leveldb(#[from] leveldb::error::Error),
    #[error(transparent)]
    Utf8Error(string::FromUtf8Error),
    #[error("Runtime: {0}")]
    RuntimeError(String),
    #[error("{0}")]
    PoisonError(String),
    #[error("{0}")]
    SendError(String),
}

impl<T> From<sync::PoisonError<T>> for Error {
    fn from(err: sync::PoisonError<T>) -> Self {
        Self::PoisonError(err.to_string())
    }
}

impl<T> convert::From<sync::mpsc::SendError<T>> for Error {
    fn from(err: sync::mpsc::SendError<T>) -> Self {
        Self::SendError(err.to_string())
    }
}

impl From<bitcoin::hashes::FromSliceError> for Error {
    fn from(err: bitcoin::hashes::FromSliceError) -> Self {
        Self::InvalidHash(err.to_string())
    }
}

impl From<bitcoin::hashes::hex::error::OddLengthStringError> for Error {
    fn from(err: bitcoin::hashes::hex::error::OddLengthStringError) -> Self {
        Self::InvalidHash(err.to_string())
    }
}

impl From<bitcoin::hashes::hex::error::HexToArrayError> for Error {
    fn from(err: bitcoin::hashes::hex::error::HexToArrayError) -> Self {
        Self::InvalidHash(err.to_string())
    }
}

impl From<bitcoin::hashes::hex::error::HexToBytesError> for Error {
    fn from(err: bitcoin::hashes::hex::error::HexToBytesError) -> Self {
        Self::InvalidHash(err.to_string())
    }
}

impl From<bitcoin::hashes::hex::error::InvalidCharError> for Error {
    fn from(err: bitcoin::hashes::hex::error::InvalidCharError) -> Self {
        Self::InvalidHash(err.to_string())
    }
}
