//!
//! Integration Test
//!
//! Test multiple APIs. Cross checking results between each other.
//!
#[cfg(test)]
mod iterator_tests {
    use bitcoin::{Block, Transaction};
    use bitcoin_explorer::{
        BitcoinDB, CompactBlock, CompactTransaction, FullBlock, FullTransaction, SConnectedBlock,
        SConnectedTransaction,
    };
    use std::path::PathBuf;

    const END: usize = 700000;

    /// utility function
    fn get_test_db() -> BitcoinDB {
        let mut crate_root_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        crate_root_dir.push("./resources/tests/Bitcoin");
        BitcoinDB::new(&crate_root_dir, true).unwrap()
    }

    #[test]
    /// iterate through all blocks, check order and correctness
    fn test_iter_block() {
        let db = get_test_db();

        let mut h = 0;
        for blk in db.block_iter::<CompactBlock>(0, END) {
            let blk_ref = db.get_block::<CompactBlock>(h).unwrap();
            assert_eq!(blk, blk_ref);
            h += 1;
        }
        // assert that all blocks are read
        assert_eq!(h, db.get_block_count())
    }

    #[test]
    /// iterate through part of the chain
    fn test_iter_block_early_end() {
        let db = get_test_db();
        let start = 100;
        let early_end = 100000;

        let mut h = start;
        for blk in db.block_iter::<CompactBlock>(start, early_end) {
            let blk_ref = db.get_block::<CompactBlock>(h).unwrap();
            assert_eq!(blk, blk_ref);
            h += 1;
        }
        assert_eq!(h, early_end)
    }

    #[test]
    /// ensure that the iterator can be dropped after breaking loop
    fn test_iter_block_break() {
        let db = get_test_db();
        let break_height = 100000;

        let mut some_blk = None;
        for (i, blk) in db.block_iter::<CompactBlock>(0, END).enumerate() {
            some_blk = Some(blk);
            if i == break_height {
                break;
            }
        }
        assert_eq!(some_blk, Some(db.get_block(break_height).unwrap()))
    }

    #[test]
    /// ensure that `get_transaction` responds with correct transaction
    fn test_get_transactions() {
        let db = get_test_db();
        let early_end = 100000;

        for blk in db.block_iter::<Block>(0, early_end) {
            for tx in blk.txdata {
                assert_eq!(
                    db.get_transaction::<Transaction>(tx.compute_txid())
                        .unwrap(),
                    tx
                );
            }
        }
    }

    #[test]
    /// iterate through all blocks
    fn test_iter_connected() {
        let db = get_test_db();

        let mut h = 0;
        for blk in db.connected_block_iter::<SConnectedBlock>(END) {
            // check that blocks are produced in correct order
            assert_eq!(blk.header, db.get_block::<CompactBlock>(h).unwrap().header);
            h += 1;
        }
        // assert that all blocks are read
        assert_eq!(h, db.get_block_count())
    }

    #[test]
    /// iterate through part of the chain
    fn test_iter_connected_early_end() {
        let db = get_test_db();
        let early_end = 100000;

        let mut h = 0;
        for blk in db.connected_block_iter::<SConnectedBlock>(early_end) {
            let blk_ref = db.get_connected_block::<SConnectedBlock>(h).unwrap();
            assert_eq!(blk, blk_ref);
            h += 1;
        }
        assert_eq!(h, early_end)
    }

    #[test]
    /// ensure that the iterator can be dropped after breaking loop
    fn test_iter_connected_break() {
        let db = get_test_db();
        let break_height = 100000;

        let mut some_blk = None;
        for (i, blk) in db.connected_block_iter::<SConnectedBlock>(END).enumerate() {
            some_blk = Some(blk);
            if i == break_height {
                break;
            }
        }
        assert_eq!(
            some_blk,
            Some(
                db.get_connected_block::<SConnectedBlock>(break_height)
                    .unwrap()
            )
        )
    }

    #[test]
    /// ensure that `get_connected_transaction` responds with correct transaction
    fn test_get_connected_transactions() {
        let db = get_test_db();
        let early_end = 100000;

        for blk in db.connected_block_iter::<SConnectedBlock>(early_end) {
            for tx in blk.txdata {
                let connected_tx = db
                    .get_connected_transaction::<SConnectedTransaction>(tx.txid)
                    .unwrap();
                let unconnected_stx = db.get_transaction::<CompactTransaction>(tx.txid).unwrap();
                let unconnected_ftx = db.get_transaction::<FullTransaction>(tx.txid).unwrap();
                assert_eq!(connected_tx.input.len(), unconnected_stx.input.len());
                assert_eq!(connected_tx.input.len(), unconnected_ftx.input.len());
                assert_eq!(connected_tx, tx);
            }
        }
    }

    #[test]
    /// assert that coinbase input has zero length
    fn test_coinbase_input() {
        let db = get_test_db();

        for blk in db.block_iter::<CompactBlock>(0, END) {
            assert_eq!(blk.txdata.first().unwrap().input.len(), 0);
        }

        for blk in db.block_iter::<FullBlock>(0, END) {
            assert_eq!(blk.txdata.first().unwrap().input.len(), 0);
        }
    }

    #[test]
    fn test_iter_block_heights() {
        let db = get_test_db();
        let test_heights = vec![3, 6, 2, 7, 1, 8, 3, 8, 1, 8, 2, 7, 21];
        let blocks_ref: Vec<CompactBlock> = test_heights
            .iter()
            .map(|h| db.get_block::<CompactBlock>(*h).unwrap())
            .collect();
        let blocks: Vec<CompactBlock> = db.iter_heights::<CompactBlock, _>(test_heights).collect();
        assert_eq!(blocks, blocks_ref)
    }
}
