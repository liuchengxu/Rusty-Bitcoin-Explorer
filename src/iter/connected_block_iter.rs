use crate::api::BitcoinDB;
use crate::iter::fetch_connected_async::{connect_outpoints, update_unspent_cache};
#[cfg(not(feature = "on-disk-utxo"))]
use crate::iter::util::VecMap;
use crate::parser::block_types::connected_block::ConnectedBlock;
#[cfg(not(feature = "on-disk-utxo"))]
use crate::parser::block_types::connected_block::ConnectedTx;
#[cfg(not(feature = "on-disk-utxo"))]
use bitcoin::Txid;
#[cfg(not(feature = "on-disk-utxo"))]
use hash_hasher::HashedMap;
#[cfg(feature = "on-disk-utxo")]
use log::error;
#[cfg(feature = "on-disk-utxo")]
use num_cpus;
use par_iter_sync::{IntoParallelIteratorSync, ParIterSync};
#[cfg(feature = "on-disk-utxo")]
use rocksdb::{Options, SliceTransform, DB};
use std::sync::Arc;
#[cfg(not(feature = "on-disk-utxo"))]
use std::sync::Mutex;
#[cfg(feature = "on-disk-utxo")]
use tempdir::TempDir;

/// 32 (txid) + 4 (i32 out n)
#[cfg(feature = "on-disk-utxo")]
pub(crate) const KEY_LENGTH: u32 = 32 + 4;

/// iterate through blocks, and connecting outpoints.
pub struct ConnectedBlockIter<TBlock> {
    inner: ParIterSync<TBlock>,
    #[cfg(feature = "on-disk-utxo")]
    #[allow(dead_code)]
    cache: Option<TempDir>,
}

impl<TBlock> ConnectedBlockIter<TBlock>
where
    TBlock: 'static + ConnectedBlock + Send,
{
    /// the worker threads are dispatched in this `new` constructor!
    pub fn new(db: &BitcoinDB, end: usize) -> Self {
        // UTXO cache
        #[cfg(not(feature = "on-disk-utxo"))]
        let unspent: Arc<
            Mutex<HashedMap<Txid, Arc<Mutex<VecMap<<TBlock::Tx as ConnectedTx>::TOut>>>>>,
        > = Arc::new(Mutex::new(HashedMap::default()));
        #[cfg(feature = "on-disk-utxo")]
        let cache_dir = {
            match TempDir::new("rocks_db") {
                Ok(tempdir) => tempdir,
                Err(e) => {
                    error!("failed to create rocksDB tempdir for UTXO: {}", e);
                    return ConnectedBlockIter::null();
                }
            }
        };
        #[cfg(feature = "on-disk-utxo")]
        let unspent = {
            let mut options = Options::default();
            // create table
            options.create_if_missing(true);
            // config to more jobs
            options.set_max_background_jobs(num_cpus::get() as i32);
            // configure mem-table to a large value (256 MB)
            options.set_write_buffer_size(0x10000000);
            // configure l0 and l1 size, let them have the same size (1 GB)
            options.set_level_zero_file_num_compaction_trigger(4);
            options.set_max_bytes_for_level_base(0x40000000);
            // 256MB file size
            options.set_target_file_size_base(0x10000000);
            // use a smaller compaction multiplier
            options.set_max_bytes_for_level_multiplier(4.0);
            // use 8-byte prefix (2 ^ 64 is far enough for transaction counts)
            options.set_prefix_extractor(SliceTransform::create_fixed_prefix(8));
            Arc::new(match DB::open(&options, &cache_dir) {
                Ok(db) => db,
                Err(e) => {
                    error!("failed to create temp rocksDB for UTXO: {}", e);
                    return ConnectedBlockIter::null();
                }
            })
        };
        // all tasks
        let heights = 0..end;
        let db_copy = db.clone();
        let unspent_copy = unspent.clone();

        #[cfg(feature = "on-disk-utxo")]
        let output_iterator = heights.into_par_iter_sync(move |height| {
            update_unspent_cache(&unspent_copy, &db_copy, height)
        });

        #[cfg(not(feature = "on-disk-utxo"))]
        let output_iterator = heights.into_par_iter_sync(move |height| {
            update_unspent_cache::<TBlock>(&unspent_copy, &db_copy, height)
        });

        let output_iterator =
            output_iterator.into_par_iter_sync(move |blk| connect_outpoints(&unspent, blk));

        ConnectedBlockIter {
            inner: output_iterator,
            // cache dir will be deleted when ConnectedBlockIter is dropped
            #[cfg(feature = "on-disk-utxo")]
            cache: Some(cache_dir),
        }
    }

    #[cfg(feature = "on-disk-utxo")]
    fn null() -> Self {
        ConnectedBlockIter {
            inner: Vec::new().into_par_iter_sync(|_: usize| Err(())),
            #[cfg(feature = "on-disk-utxo")]
            cache: None,
        }
    }
}

impl<TBlock> Iterator for ConnectedBlockIter<TBlock> {
    type Item = TBlock;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

#[cfg(test)]
#[cfg(feature = "on-disk-utxo")]
mod test_empty {
    use crate::{ConnectedBlockIter, SConnectedBlock};

    #[test]
    fn test_empty() {
        let mut empty = ConnectedBlockIter::null();
        for _ in 0..100 {
            let b: Option<SConnectedBlock> = empty.next();
            assert!(b.is_none());
        }
    }
}
