#[cfg(not(feature = "on-disk-utxo"))]
pub use in_mem_utxo::{connect_outpoints, update_unspent_cache};
#[cfg(feature = "on-disk-utxo")]
pub use on_disk_utxo::{connect_outpoints, update_unspent_cache};

#[cfg(feature = "on-disk-utxo")]
mod on_disk_utxo {
    use crate::iter::connected_block_iter::KEY_LENGTH;
    use crate::parser::block_types::connected_block::{ConnectedBlock, ConnectedTx};
    use crate::BitcoinDB;
    use bitcoin::consensus::{Decodable, Encodable};
    use bitcoin::{Block, TxOut, Txid};
    use log::error;
    use rocksdb::{WriteBatch, DB};
    use std::sync::Arc;

    #[inline(always)]
    fn txo_key(txid: Txid, n: u32) -> Vec<u8> {
        use bitcoin::hashes::Hash;

        let mut bytes = Vec::with_capacity(KEY_LENGTH as usize);
        bytes.extend(txid.as_byte_array());
        bytes.extend(n.to_ne_bytes());
        bytes
    }

    #[inline(always)]
    fn txo_to_u8(txo: &TxOut) -> Vec<u8> {
        let mut bytes = Vec::new();
        txo.consensus_encode(&mut bytes).unwrap();
        bytes
    }

    #[inline(always)]
    fn txo_from_u8(bytes: Vec<u8>) -> Option<TxOut> {
        match TxOut::consensus_decode(&mut bytes.as_slice()) {
            Ok(txo) => Some(txo),
            Err(_) => None,
        }
    }

    /// read block, update UTXO cache, return block
    pub fn update_unspent_cache(
        unspent: &Arc<DB>,
        db: &BitcoinDB,
        height: usize,
    ) -> Result<Block, ()> {
        match db.get_block::<Block>(height) {
            Ok(block) => {
                let mut batch = WriteBatch::default();

                // insert new transactions
                for tx in block.txdata.iter() {
                    // clone outputs
                    let txid = tx.compute_txid();

                    for (n, o) in (0_u32..).zip(tx.output.iter()) {
                        let key = txo_key(txid, n);
                        let value = txo_to_u8(o);
                        batch.put(key, value);
                    }
                }
                match unspent.write_without_wal(batch) {
                    Ok(_) => Ok(block),
                    Err(e) => {
                        error!("failed to write UTXO to cache, error: {}", e);
                        Err(())
                    }
                }
            }

            Err(_) => Err(()),
        }
    }

    /// fetch_block_connected, thread safe
    pub fn connect_outpoints<TBlock>(unspent: &Arc<DB>, block: Block) -> Result<TBlock, ()>
    where
        TBlock: ConnectedBlock,
    {
        let block_hash = block.header.block_hash();
        let mut output_block = TBlock::from(block.header, block_hash);

        // collect rocks db keys
        let mut keys = Vec::new();

        for tx in block.txdata.iter() {
            for input in tx.input.iter() {
                // skip coinbase transaction
                if input.previous_output.is_null() {
                    continue;
                }

                keys.push(txo_key(
                    input.previous_output.txid,
                    input.previous_output.vout,
                ));
            }
        }

        // get utxo
        let tx_outs = unspent.multi_get(keys.clone());

        // remove keys
        for key in keys {
            match unspent.delete(&key) {
                Ok(_) => {}
                Err(e) => {
                    error!("failed to remove key {:?}, error: {}", &key, e);
                    return Err(());
                }
            }
        }

        // pointer to record read position in tx_outs
        let mut pos = 0;

        for tx in block.txdata {
            let mut output_tx: TBlock::Tx = ConnectedTx::from(&tx);

            // spend new inputs
            for input in tx.input {
                // skip coinbase transaction
                if input.previous_output.is_null() {
                    continue;
                }

                let prev_txo = match tx_outs.get(pos).unwrap() {
                    Ok(bytes) => match bytes {
                        None => None,
                        Some(bytes) => txo_from_u8(bytes.to_vec()),
                    },
                    Err(_) => None,
                };

                if let Some(out) = prev_txo {
                    output_tx.add_input(out.into());
                    pos += 1;
                } else {
                    error!("cannot find previous outpoint, bad data");
                    return Err(());
                }
            }
            output_block.add_tx(output_tx);
        }
        Ok(output_block)
    }
}

#[cfg(not(feature = "on-disk-utxo"))]
mod in_mem_utxo {
    use crate::iter::util::VecMap;
    use crate::parser::block_types::connected_block::{ConnectedBlock, ConnectedTx};
    use crate::BitcoinDB;
    use bitcoin::{Block, Txid};
    use hash_hasher::HashedMap;
    use log::error;
    #[cfg(debug_assertions)]
    use log::warn;
    use std::sync::{Arc, Mutex};

    /// read block, update UTXO cache, return block
    pub fn update_unspent_cache<TBlock>(
        unspent: &Arc<
            Mutex<HashedMap<Txid, Arc<Mutex<VecMap<<TBlock::Tx as ConnectedTx>::TOut>>>>>,
        >,
        db: &BitcoinDB,
        height: usize,
    ) -> Result<Block, ()>
    where
        TBlock: ConnectedBlock,
    {
        match db.get_block::<Block>(height) {
            Ok(block) => {
                let mut new_unspent_cache = Vec::with_capacity(block.txdata.len());

                // insert new transactions
                for tx in block.txdata.iter() {
                    // clone outputs
                    let txid = tx.compute_txid();
                    let mut outs: Vec<Option<Box<<TBlock::Tx as ConnectedTx>::TOut>>> =
                        Vec::with_capacity(tx.output.len());
                    for o in tx.output.iter() {
                        outs.push(Some(Box::new(o.clone().into())));
                    }

                    // update unspent cache
                    let outs: VecMap<<TBlock::Tx as ConnectedTx>::TOut> =
                        VecMap::from_vec(outs.into_boxed_slice());
                    let new_unspent = Arc::new(Mutex::new(outs));

                    // the new transaction should not be in unspent
                    #[cfg(debug_assertions)]
                    if unspent.lock().unwrap().contains_key(&txid) {
                        warn!("found duplicate key {}", &txid);
                    }

                    new_unspent_cache.push((txid, new_unspent));
                }
                unspent.lock().unwrap().extend(new_unspent_cache);
                // if some exception happens in lower stream
                Ok(block)
            }
            Err(_) => Err(()),
        }
    }

    /// fetch_block_connected, thread safe
    pub fn connect_outpoints<TBlock>(
        unspent: &Arc<
            Mutex<HashedMap<Txid, Arc<Mutex<VecMap<<TBlock::Tx as ConnectedTx>::TOut>>>>>,
        >,
        block: Block,
    ) -> Result<TBlock, ()>
    where
        TBlock: ConnectedBlock,
    {
        let block_hash = block.header.block_hash();
        let mut output_block = TBlock::from(block.header, block_hash);

        for tx in block.txdata {
            let mut output_tx: TBlock::Tx = ConnectedTx::from(&tx);

            // spend new inputs
            for input in tx.input {
                // skip coinbase transaction
                if input.previous_output.is_null() {
                    continue;
                }

                let prev_txid = &input.previous_output.txid;
                let n = input.previous_output.vout as usize;

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
                    let (tx_out, is_empty) = {
                        let mut prev_tx_lock = prev_tx.lock().unwrap();
                        let tx_out = prev_tx_lock.remove(n);
                        let is_empty = prev_tx_lock.is_empty();
                        (tx_out, is_empty)
                    };
                    // remove a key immediately when the key contains no transaction
                    if is_empty {
                        unspent.lock().unwrap().remove(prev_txid);
                    }
                    if let Some(out) = tx_out {
                        output_tx.add_input(*out);
                    } else {
                        error!("cannot find previous outpoint, bad data");
                        return Err(());
                    }
                } else {
                    error!("cannot find previous transactions, bad data");
                    return Err(());
                }
            }
            output_block.add_tx(output_tx);
        }
        Ok(output_block)
    }
}
