use crate::iter::util::{Compress, DBCopy, VecMap};
use crate::parser::proto::connected_proto::{BlockConnectable, TxConnectable};
use log::warn;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use bitcoin::Block;
use hash_hasher::HashedMap;

///
/// read block, update cache
///
pub(crate) fn insert_outputs<TBlock>(
    unspent: &Arc<
        Mutex<HashedMap<u128, Arc<Mutex<VecMap<<TBlock::Tx as TxConnectable>::TOut>>>>>,
    >,
    db: &DBCopy,
    height: u32,
    error_state: &Arc<AtomicBool>,
    channel: &SyncSender<Block>,
) -> bool
where
    TBlock: BlockConnectable,
{
    // stop new tasks from loading when error
    if error_state.load(Ordering::SeqCst) {
        return false;
    }

    if let Some(index) = db.block_index.records.get(height as usize) {
        match db.blk_file.read_block(index.n_file, index.n_data_pos) {
            Ok(block) => {

                let mut new_unspent_cache = Vec::with_capacity(block.txdata.len());

                // insert new transactions
                for tx in block.txdata.iter() {

                    // clone outputs
                    let txid = tx.txid();
                    let mut outs: Vec<Option<<TBlock::Tx as TxConnectable>::TOut>> =
                        Vec::with_capacity(tx.output.len());
                    for o in tx.output.iter() {
                        outs.push(Some(o.clone().into()));
                    }

                    // update unspent cache
                    let outs: VecMap<<TBlock::Tx as TxConnectable>::TOut> =
                        VecMap::from_vec(outs.into_boxed_slice());
                    let new_unspent: Arc<Mutex<VecMap<<TBlock::Tx as TxConnectable>::TOut>>> =
                        Arc::new(Mutex::new(outs));
                    let txid_compressed = txid.compress();
                    // the new transaction should not be in unspent
                    #[cfg(debug_assertions)]
                    if unspent.lock().unwrap().contains_key(&txid_compressed) {
                        warn!("found duplicate key {}", &txid);
                    }
                    //
                    if error_state.load(Ordering::SeqCst) {
                        return false;
                    }
                    new_unspent_cache.push((txid_compressed, new_unspent));
                }
                {
                    let mut lock = unspent.lock().unwrap();
                    lock.extend(new_unspent_cache);
                    channel.send(block).unwrap();
                    // release unspent cache
                }
                true
            }
            Err(_) => {
                // set error_state to true
                mutate_result_error(error_state);
                false
            }
        }
    } else {
        // set error_state to true
        mutate_result_error(error_state);
        false
    }
}

///
/// fetch_block_connected, thread safe
///
pub(crate) fn consume_outputs<TBlock>(
    unspent: &Arc<
        Mutex<HashedMap<u128, Arc<Mutex<VecMap<<TBlock::Tx as TxConnectable>::TOut>>>>>,
    >,
    error_state: &Arc<AtomicBool>,
    sender: &SyncSender<TBlock>,
    block: Block,
) -> bool
    where
        TBlock: BlockConnectable,
{
    // stop new tasks from loading when error
    if error_state.load(Ordering::SeqCst) {
        return false;
    }

    let block_hash = block.header.block_hash();
    let mut output_block = TBlock::from(block.header, block_hash);

    for tx in block.txdata {
        let mut output_tx: TBlock::Tx = TxConnectable::from(&tx);

        // spend new inputs
        for input in tx.input {
            // skip coinbase transaction
            if input.previous_output.is_null() {
                continue;
            }

            let prev_txid = &input.previous_output.txid.compress();
            let n = *&input.previous_output.vout as usize;

            // temporarily lock unspent
            let prev_tx = {
                let prev_tx = unspent.lock().unwrap();
                match prev_tx.get(prev_txid) {
                    None => None,
                    Some(tx) => Some(tx.clone()),
                }
            };
            if let Some(prev_tx) = prev_tx {
                // temporarily lock prev_tx
                let tx_out = {
                    let mut prev_tx_lock = prev_tx.lock().unwrap();
                    let out = prev_tx_lock.remove(n);
                    // remove a key immediately when the key contains no transaction
                    if prev_tx_lock.is_empty() {
                        unspent.lock().unwrap().remove(prev_txid);
                    }
                    out
                };
                if let Some(out) = tx_out {
                    output_tx.add_input(out);
                } else {
                    warn!("cannot find previous outpoint, bad data");
                    // only increment result lock
                    mutate_result_error(error_state);
                    return false;
                }
            } else {
                warn!("cannot find previous transactions, bad data");
                // only increment result lock
                mutate_result_error(error_state);
                return false;
            }
        }
        output_block.add_tx(output_tx);
    }

    if error_state.load(Ordering::SeqCst) {
        return false;
    }
    sender.send(output_block).unwrap();
    true
}

#[inline]
fn mutate_result_error(error_state: &Arc<AtomicBool>) {
    error_state.fetch_or(true, Ordering::SeqCst);
}
