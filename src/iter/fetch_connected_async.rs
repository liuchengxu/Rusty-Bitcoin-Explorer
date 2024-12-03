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
    use rocksdb::{WriteBatch, DB};
    use std::sync::Arc;

    #[inline(always)]
    fn txout_key(txid: Txid, n: u32) -> Vec<u8> {
        use bitcoin::hashes::Hash;

        let mut bytes = Vec::with_capacity(KEY_LENGTH as usize);
        bytes.extend(txid.as_byte_array());
        bytes.extend(n.to_ne_bytes());
        bytes
    }

    #[inline(always)]
    fn encode_txout(txo: &TxOut) -> Vec<u8> {
        let mut bytes = Vec::new();
        txo.consensus_encode(&mut bytes).unwrap();
        bytes
    }

    /// read block, update UTXO cache, return block
    pub fn update_unspent_cache(
        unspent: &Arc<DB>,
        db: &BitcoinDB,
        height: usize,
    ) -> Result<Block, ()> {
        let block = db.get_block::<Block>(height).map_err(|_| ())?;
        let mut batch = WriteBatch::default();

        // insert new transactions
        for tx in block.txdata.iter() {
            // clone outputs
            let txid = tx.compute_txid();

            for (n, o) in (0_u32..).zip(tx.output.iter()) {
                let key = txout_key(txid, n);
                let value = encode_txout(o);
                batch.put(key, value);
            }
        }
        match unspent.write_without_wal(batch) {
            Ok(_) => Ok(block),
            Err(e) => {
                log::error!("failed to write UTXO to cache, error: {}", e);
                Err(())
            }
        }
    }

    /// fetch_block_connected, thread safe
    pub fn connect_outpoints<B>(unspent: &Arc<DB>, block: Block) -> Result<B, ()>
    where
        B: ConnectedBlock,
    {
        let block_hash = block.header.block_hash();
        let mut output_block = B::from(block.header, block_hash);

        // collect rocks db keys
        let keys = block
            .txdata
            .iter()
            .flat_map(|tx| {
                tx.input.iter().filter_map(|input| {
                    if input.previous_output.is_null() {
                        None
                    } else {
                        Some(txout_key(
                            input.previous_output.txid,
                            input.previous_output.vout,
                        ))
                    }
                })
            })
            .collect::<Vec<_>>();

        // get utxo
        let tx_outs = unspent.multi_get(keys.clone());

        // remove keys
        for key in keys {
            if let Err(e) = unspent.delete(&key) {
                log::error!("failed to remove key {key:?}: {e}");
                return Err(());
            }
        }

        // pointer to record read position in tx_outs
        let mut pos = 0;

        for tx in block.txdata {
            let mut output_tx: B::Tx = ConnectedTx::from(&tx);

            // spend new inputs
            for input in tx.input {
                // skip coinbase transaction
                if input.previous_output.is_null() {
                    continue;
                }

                let prev_txo = match tx_outs.get(pos).unwrap() {
                    Ok(Some(bytes)) => TxOut::consensus_decode(&mut bytes.as_slice()).ok(),
                    Ok(None) => None,
                    Err(_) => None,
                };

                if let Some(out) = prev_txo {
                    output_tx.add_input(out.into());
                    pos += 1;
                } else {
                    log::error!("cannot find previous outpoint, bad data");
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
    use std::sync::{Arc, Mutex};

    #[cfg(not(feature = "on-disk-utxo"))]
    type InMemoryUtxoCache<B> = Arc<
        Mutex<
            HashedMap<Txid, Arc<Mutex<VecMap<<<B as ConnectedBlock>::Tx as ConnectedTx>::TxOut>>>>,
        >,
    >;

    /// read block, update UTXO cache, return block
    pub fn update_unspent_cache<B>(
        unspent: &InMemoryUtxoCache<B>,
        db: &BitcoinDB,
        height: usize,
    ) -> Result<Block, ()>
    where
        B: ConnectedBlock,
    {
        let block = db.get_block::<Block>(height).map_err(|_| ())?;
        let mut new_unspent_cache = Vec::with_capacity(block.txdata.len());

        // insert new transactions
        for tx in block.txdata.iter() {
            // clone outputs
            let txid = tx.compute_txid();
            let outs: Vec<Option<Box<<B::Tx as ConnectedTx>::TxOut>>> = tx
                .output
                .iter()
                .map(|o| Some(Box::new(o.clone().into())))
                .collect();

            // update unspent cache
            let outs: VecMap<<B::Tx as ConnectedTx>::TxOut> =
                VecMap::from_vec(outs.into_boxed_slice());
            let new_unspent = Arc::new(Mutex::new(outs));

            // the new transaction should not be in unspent
            #[cfg(debug_assertions)]
            if unspent.lock().unwrap().contains_key(&txid) {
                log::warn!("found duplicate key {}", &txid);
            }

            new_unspent_cache.push((txid, new_unspent));
        }
        unspent.lock().unwrap().extend(new_unspent_cache);
        // if some exception happens in lower stream
        Ok(block)
    }

    /// fetch_block_connected, thread safe
    pub fn connect_outpoints<B>(unspent: &InMemoryUtxoCache<B>, block: Block) -> Result<B, ()>
    where
        B: ConnectedBlock,
    {
        let block_hash = block.header.block_hash();
        let mut output_block = B::from(block.header, block_hash);

        for tx in block.txdata {
            let mut output_tx: B::Tx = ConnectedTx::from(&tx);

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
                    prev_tx.get(prev_txid).cloned()
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
                        log::error!("cannot find previous outpoint, bad data");
                        return Err(());
                    }
                } else {
                    log::error!("cannot find previous transactions, bad data");
                    return Err(());
                }
            }
            output_block.add_tx(output_tx);
        }

        Ok(output_block)
    }
}
