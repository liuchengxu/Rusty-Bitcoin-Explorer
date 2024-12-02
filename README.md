# bitcoin-explorer

[![Rust](https://github.com/liuchengxu/Rusty-Bitcoin-Explorer/actions/workflows/rust.yml/badge.svg)](https://github.com/liuchengxu/Rusty-Bitcoin-Explorer/actions/workflows/rust.yml)

This is a fork of https://github.com/Congyuwang/Rusty-Bitcoin-Explorer primarily for supporting [Subcoin](https://github.com/subcoin-project/subcoin).

--------------

`bitcoin_explorer` is an efficient library for decoding transaction information from
bitcoin blockchain.

Support bitcoin MainNet, might support other networks in the future.

## Features

### **1. Block & Script Decoding**

- Query blocks based on block heights or block hash.
- Support `tx_index=1`.
- Find input addresses using UTXO cache (`connected_block_iter()`).

### **2. Concurrency + Iterator + Sequential Output**

- Fast concurrent deserializing but producing sequential output.
- Native Iterator interface (support `for in` syntax).

### **3. Small Memory Footprint (< 4 GB RAM)**

- Use a fast on-disk UTXO storage (RocksDB).

### **4. Build for Rust + Python (Multi-OS PyPI wheels)**

- Built and published PyPI wheels for `python 3.6-3.10` across `Windows x86/x64`, `MacOS x86_64/arm64`, and `Linux x86_64`.
- Rust library on [crates.io](https://crates.io) called *bitcoin-explorer*.

## Documentation

See [Rust Documentation](https://docs.rs/bitcoin-explorer/)

## Examples

### Get total number of blocks and transactions available on disk
```rust
use bitcoin_explorer::{BitcoinDB, FullConnectedBlock, SConnectedBlock};
use std::path::Path;

fn main() {

    let path = Path::new("/Users/me/bitcoin");
    let db = BitcoinDB::new(path, false).unwrap();

    let block_count = db.get_block_count();

    let total_number_of_transactions = (0..block_count)
        .map(|i| db.get_header(i).unwrap().n_tx)
        .sum::<u32>();

}

```

### Get a block (i.e., see doc for what is full/simple format (`FullBlock`/`CompactBlock`) )

```rust
use bitcoin_explorer::{BitcoinDB, FullBlock, CompactBlock, Block};
use std::path::Path;

fn main() {
    let path = Path::new("/Users/me/bitcoin");

    // launch without reading txindex
    let db = BitcoinDB::new(path, false).unwrap();

    // get block of height 600000 (in different formats)
    let block: Block = db.get_block(600000).unwrap();
    let block: FullBlock = db.get_block(600000).unwrap();
    let block: CompactBlock = db.get_block(600000).unwrap();
}
```

### Get a transaction (in different formats)

Note: this requires building tx index with `--txindex=1` flag using Bitcoin Core.

```rust
use bitcoin_explorer::{BitcoinDB, Transaction, FullTransaction, CompactTransaction, Txid, FromHex};
use std::path::Path;

fn main() {
    let path = Path::new("/Users/me/bitcoin");

    // !!must launch with txindex=true!!
    let db = BitcoinDB::new(path, true).unwrap();

    // get transaction
    // e3bf3d07d4b0375638d5f1db5255fe07ba2c4cb067cd81b84ee974b6585fb468
    let txid_str = "e3bf3d07d4b0375638d5f1db5255fe07ba2c4cb067cd81b84ee974b6585fb468";
    let txid = Txid::from_hex(txid_str).unwrap();

    // get transactions in different formats
    let tx: Transaction = db.get_transaction(&txid).unwrap();
    let tx: FullTransaction = db.get_transaction(&txid).unwrap();
    let tx: CompactTransaction = db.get_transaction(&txid).unwrap();
}
```

### Iterate through all blocks (in different formats)

```rust
use bitcoin_explorer::{BitcoinDB, Block, CompactBlock, FullBlock};
use std::path::Path;

fn main() {
    let path = Path::new("/Users/me/bitcoin");

    // launch without reading txindex
    let db = BitcoinDB::new(path, false).unwrap();

    // iterate over block from 0 to 1000
    for block in db.block_iter::<Block>(0, 1000) {
        for tx in block.txdata {
            println!("do something for this transaction");
        }
    }

    // iterate over block from 1000 to end
    for block in db.block_iter::<FullBlock>(1000, db.get_block_count()) {
        for tx in block.txdata {
            println!("do something for this transaction");
        }
    }

    // iterate over block from 0 to end
    for block in db.block_iter::<CompactBlock>(0, db.get_block_count()) {
        for tx in block.txdata {
            println!("do something for this transaction");
        }
    }
}
```

### Iterate through all blocks with Input Addresses Found (`ConnectedBlock`)

```rust
use bitcoin_explorer::{BitcoinDB, FullConnectedBlock, SConnectedBlock};
use std::path::Path;

fn main() {

    let path = Path::new("/Users/me/bitcoin");

    // launch without reading txindex
    let db = BitcoinDB::new(path, false).unwrap();
    let end = db.get_block_count();

    // iterate over all blocks found (simple connected format)
    for block in db.connected_block_iter::<SConnectedBlock>(end) {
        for tx in block.txdata {
            println!("do something for this transaction");
        }
    }
}
```

## Hardware Requirements

### Memory Requirement

Memory requirement: 8 GB physical RAM.

### Disk Requirement

SSD for better performance.

## Benchmarking

- OS: `x86_64` Windows 10
- CPU: Intel i7-9700 @ 3.00GHZ (4-core, 8-threads)
- Memory: 16 GB 2667 Mhz
- Disk: WDC SN730 512GB (SSD)

### Iteration Through All Blocks (0 - 700000)

```rust
db.block_iter::<CompactBlock>(0, 700000)
```

- Time: about 10 minutes
- Peak Memory: <= 500 MB

### Iteration Through All Blocks (0 - 700000) With Input Addresses 

```rust
db.connected_block_iter::<SConnectedBlock>(700000)
```

#### Using default configuration

Compile with default features (Cargo.toml):

```toml
bitcoin-explorer = "^1.2"
```

- Time: about 2.5 hours
- Peak Memory: 4 GB

#### Using non-default configuration (large RAM for good performance)

Compile with non-default features (Cargo.toml):

```toml
bitcoin-explorer = { version = "^1.2", default-features = false }
```

- Time: about 30 minutes
- Peak Memory: 32 GB

## Notes

### Compatibility

This package deals with the binary file of another software `Bitcoin Core`.
It might not be compatible with older Bitcoin Core versions.

Tested on
`Bitcoin Core version v0.21.1.0-g194b9b8792d9b0798fdb570b79fa51f1d1f5ebaf
Copyright (C) 2009-2020 The Bitcoin Core developers`.

### Non-Default Feature (In-Memory-UTXO cache)

If you have more than 32 GB memory, you might try `default-features = false`
for faster performance on `db.connected_block_iter()`
```toml
bitcoin-explorer = { version = "^1.2", default-features = false }
```
