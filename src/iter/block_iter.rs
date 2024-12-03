//! View development note of connected_block_iter.rs for implementation
//! details of block_iter.rs, which follows similar principles.

use crate::api::BitcoinDB;
use bitcoin::Block;
use par_iter_sync::{IntoParallelIteratorSync, ParIterSync};

pub struct BlockIter<B>(ParIterSync<B>);

impl<B> BlockIter<B>
where
    B: From<Block> + Send + 'static,
{
    /// the worker threads are dispatched in this `new` constructor!
    pub fn new<T>(db: &BitcoinDB, heights: T) -> Self
    where
        T: IntoIterator<Item = usize> + Send + 'static,
        <T as IntoIterator>::IntoIter: Send + 'static,
    {
        let db = db.clone();
        Self(heights.into_par_iter_sync(move |h| db.get_block::<B>(h).map_err(|_| ())))
    }

    /// the worker threads are dispatched in this `new` constructor!
    pub fn from_range(db: &BitcoinDB, start: usize, end: usize) -> Self {
        if end <= start {
            Self::new(db, Vec::new())
        } else {
            Self::new(db, start..end)
        }
    }
}

impl<B> Iterator for BlockIter<B> {
    type Item = B;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}
