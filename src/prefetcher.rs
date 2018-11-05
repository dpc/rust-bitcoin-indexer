use common_failures::prelude::*;
use crate::prelude::*;
use std::{
    collections::HashMap,
    sync::{
        self,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

fn retry<T>(f: impl Fn() -> Result<T>) -> T {
    let delay_ms = 2000;
    loop {
        match f() {
            Err(e) => {
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                eprintln!("{}; retrying ...", e.display_causes_and_backtrace());
            }
            Ok(t) => {
                return t;
            }
        }
    }
}

type PrefetcherItem = (BlockHeight, BlockHash, BitcoinCoreBlock);

/// An iterator that yields blocks
///
/// It uses thread-pool to fetch blocks and returns them in order:
///
/// ```norust
/// 1, 2, 3, ..., n-1, n ...
/// ```
///
/// In case of an reorg, it will break the sequence and return
/// the right blocks of a new chain, again in sequence:
///
/// ```norust
/// 1, 2, 3, 4, ..., 2, 3, 4 ...
/// ```
///
pub struct Prefetcher {
    tx: crossbeam_channel::Sender<PrefetcherItem>,
    rx: crossbeam_channel::Receiver<PrefetcherItem>,
    thread_joins: Vec<std::thread::JoinHandle<()>>,
    out_of_order_items: HashMap<BlockHeight, (BlockHash, BitcoinCoreBlock)>,
    cur_height: BlockHeight,
    prev_hashes: HashMap<BlockHeight, BlockHash>,
    workers_finish: sync::Arc<AtomicBool>,
    thread_num: usize,
    rpc_info: RpcInfo,
}

impl Prefetcher {
    pub fn new(rpc_info: &RpcInfo, start: u64) -> Result<Self> {
        let thread_num = 8 * 3;
        let (tx, rx) = crossbeam_channel::bounded(thread_num);
        let workers_finish = sync::Arc::new(AtomicBool::new(false));

        let mut s = Self {
            rx,
            tx,
            rpc_info: rpc_info.to_owned(),
            thread_joins: default(),
            thread_num,
            cur_height: start,
            out_of_order_items: default(),
            workers_finish,
            prev_hashes: default(),
        };

        s.start_workers();
        Ok(s)
    }

    fn stop_workers(&mut self) {
        self.workers_finish.store(true, Ordering::SeqCst);

        while let Ok(_) = self.rx.recv() {}

        self.thread_joins.drain(..).map(|j| j.join()).for_each(drop);
    }

    fn start_workers(&mut self) {
        self.workers_finish.store(false, Ordering::SeqCst);

        let current = sync::Arc::new(AtomicUsize::new(self.cur_height as usize));
        for thread_i in 0..self.thread_num {
            self.thread_joins.push({
                std::thread::spawn({
                    let current = current.clone();
                    let rpc = self.rpc_info.to_rpc_client();
                    let tx = self.tx.clone();
                    let workers_finish = self.workers_finish.clone();
                    move || {
                        let end = retry(|| Ok(rpc.getblockcount()?));

                        while !workers_finish.load(Ordering::Relaxed) {
                            let height = current.fetch_add(1, Ordering::Relaxed) as u64;

                            if thread_i > 0 && height >= end {
                                return;
                            }

                            let (hash, block) = retry(|| {
                                let hash = rpc.get_blockhash(height)?;
                                let hex = rpc.get_block(&hash)?;

                                let bytes = hex::decode(hex)?;
                                let block: BitcoinCoreBlock = bitcoin_core::deserialize(&bytes)?;
                                Ok((hash, block))
                            });
                            tx.send((height, hash, block));
                        }
                    }
                })
            });
        }
    }

    /// Detect reorgs
    ///
    /// Track previous hashes and detect if a given block points
    /// to a different `prev_blockhash` than we recorded. That
    /// means that the previous hash we've recorded was abandoned.
    fn detected_reorg(&mut self, item: &PrefetcherItem) -> bool {
        if self.cur_height == 0 {
            if let Some(prev_hash) = self.prev_hashes.get(&(self.cur_height - 1)) {
                if prev_hash != &item.1 {
                    return true;
                }
            }
        }
        self.prev_hashes.insert(item.0, item.1.clone());
        // this is how big reorgs we're going to detect
        let window_size = 100;
        if self.cur_height > window_size {
            self.prev_hashes.remove(&(self.cur_height - window_size));
        }
        debug_assert!(self.prev_hashes.len() <= window_size as usize);

        false
    }

    /// Handle condition detected by `detected_reorg`
    ///
    /// Basically, stop all workers (discarding their work), adjust height and
    /// start workers again.
    ///
    /// This doesn't have to be blazing fast, so it isn't.
    fn reset_on_reorg(&mut self) {
        self.stop_workers();
        self.cur_height -= 1;
        self.start_workers();
    }
}

impl Iterator for Prefetcher {
    type Item = PrefetcherItem;
    fn next(&mut self) -> Option<Self::Item> {
        'retry_on_reorg: loop {
            if let Some(item) = self.out_of_order_items.remove(&self.cur_height) {
                let item = (self.cur_height, item.0, item.1);
                if self.detected_reorg(&item) {
                    self.reset_on_reorg();
                    continue 'retry_on_reorg;
                }
                self.cur_height += 1;
                return Some(item);
            }

            'wait_for_next_block: loop {
                let item = self.rx.recv().expect("Workers shouldn't disconnect");
                if item.0 == self.cur_height {
                    if self.detected_reorg(&item) {
                        self.reset_on_reorg();
                        continue 'retry_on_reorg;
                    }
                    self.cur_height += 1;
                    return Some(item);
                } else {
                    self.out_of_order_items.insert(item.0, (item.1, item.2));
                }
            }
        }
    }
}

impl Drop for Prefetcher {
    fn drop(&mut self) {
        self.stop_workers();
    }
}
