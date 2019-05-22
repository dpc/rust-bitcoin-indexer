use log::{debug, info, trace};

use crate::{prelude::*, Block, BlockHeight, Rpc, RpcBlock, RpcBlockWithPrevId, Sha256dHash};
use bitcoincore_rpc::RpcApi;
use common_failures::prelude::*;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

fn retry<T>(mut f: impl FnMut() -> Result<T>) -> T {
    let delay_ms = 100;
    let mut count = 0;
    loop {
        match f() {
            Err(e) => {
                std::thread::sleep(Duration::from_millis(delay_ms));
                if count % 1000 == 0 {
                    eprintln!("{}; retrying ...", e.display_causes_and_backtrace());
                }
                count += 1;
            }
            Ok(t) => {
                return t;
            }
        }
    }
}

impl Rpc for bitcoincore_rpc::Client {
    type Data = Box<bitcoin::Block>;
    type Id = Sha256dHash;
    const RECOMMENDED_HEAD_RETRY_DELAY_MS: u64 = 2000;
    const RECOMMENDED_ERROR_RETRY_DELAY_MS: u64 = 100;

    fn get_block_count(&self) -> Result<u64> {
        Ok(RpcApi::get_block_count(self)?)
    }

    fn get_block_id_by_height(&self, height: BlockHeight) -> Result<Option<Self::Id>> {
        match self.get_block_hash(height) {
            Err(e) => {
                if e.to_string().contains("Block height out of range") {
                    Ok(None)
                } else {
                    Err(e.into())
                }
            }
            Ok(o) => Ok(Some(o)),
        }
    }

    fn get_block_by_id(&self, hash: &Self::Id) -> Result<Option<(Self::Data, Self::Id)>> {
        let block: Box<bitcoin::Block> = match self.get_by_id(hash) {
            Err(e) => {
                if e.to_string().contains("Block height out of range") {
                    return Ok(None);
                } else {
                    return Err(e.into());
                }
            }
            Ok(o) => Box::new(o),
        };
        let prev_id = block.header.prev_blockhash;
        Ok(Some((block, prev_id)))
    }
}

/// A block fetcher from a `Rpc`
///
/// Implemented as an iterator that yields block events in order,
/// and blocks for new ones when needed.
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
/// # Architecture notes
///
/// Note that prefetcher does not have any access to the DB or
/// persistent storage, so it does not know what have been previous
/// indexed. It makes using it a bit more diffucult,
/// but the benefit is that it's much more composable and isolated.
///
/// In a sense, the `Prefetcher` is a simplest and smallest-possible
/// `Indexer`, that just does not actually index anything. It only
/// fetches blocks and detects reorgs.
pub struct Prefetcher<R>
where
    R: Rpc,
{
    rx: Option<crossbeam_channel::Receiver<RpcBlockWithPrevId<R>>>,
    /// Worker threads
    thread_joins: Vec<std::thread::JoinHandle<()>>,
    /// List of blocks that arrived out-of-order: before the block
    /// we were actually waiting for.
    out_of_order_items: HashMap<BlockHeight, RpcBlockWithPrevId<R>>,

    cur_height: BlockHeight,
    prev_hashes: BTreeMap<BlockHeight, R::Id>,
    workers_finish: Arc<AtomicBool>,
    thread_num: usize,
    rpc: Arc<R>,
    end_of_fast_sync: u64,
}

impl<R> Prefetcher<R>
where
    R: Rpc + 'static,
{
    pub fn new(rpc: Arc<R>, last_block: Option<Block<R::Id>>) -> Result<Self> {
        let thread_num = num_cpus::get() * 2;
        let workers_finish = Arc::new(AtomicBool::new(false));

        let end_of_fast_sync = retry(|| Ok(rpc.get_block_count()?));
        let mut prev_hashes = BTreeMap::default();
        let start = if let Some(h_and_hash) = last_block {
            let h = h_and_hash.height;
            prev_hashes.insert(h, h_and_hash.id);
            info!("Starting block fetcher starting at {}H", h + 1);
            h + 1
        } else {
            info!("Starting block fetcher starting at genesis block");
            0
        };

        let mut s = Self {
            rx: None,
            rpc,
            thread_joins: default(),
            thread_num,
            cur_height: start,
            out_of_order_items: default(),
            workers_finish,
            prev_hashes,
            end_of_fast_sync,
        };

        s.start_workers();
        Ok(s)
    }

    fn start_workers(&mut self) {
        self.workers_finish.store(false, Ordering::SeqCst);

        let (tx, rx) = crossbeam_channel::bounded(self.thread_num * 64);
        self.rx = Some(rx);
        let next_height = Arc::new(AtomicUsize::new(self.cur_height as usize));
        assert!(self.thread_joins.is_empty());
        for _ in 0..self.thread_num {
            self.thread_joins.push({
                std::thread::spawn({
                    let next_height = next_height.clone();
                    let rpc = self.rpc.clone();
                    let tx = tx.clone();
                    let workers_finish = self.workers_finish.clone();
                    let in_progress = Arc::new(Mutex::new(default()));
                    move || {
                        // TODO: constructor
                        let mut worker = Worker {
                            next_height,
                            workers_finish,
                            rpc,
                            tx,
                            in_progress,
                        };

                        worker.run()
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
    fn track_reorgs(&mut self, block: &RpcBlockWithPrevId<R>) -> bool {
        debug_assert_eq!(block.block.height, self.cur_height);
        if self.cur_height > 0 {
            if let Some(stored_prev_id) = self.prev_hashes.get(&(self.cur_height - 1)) {
                debug!(
                    "Reorg check: last_id {} =? current {} at {}H",
                    stored_prev_id,
                    block.prev_block_id,
                    self.cur_height - 1
                );
                if stored_prev_id != &block.prev_block_id {
                    return true;
                }
            } else if self.cur_height
                < *self
                    .prev_hashes
                    .iter()
                    .next()
                    .expect("At least one element")
                    .0
            {
                panic!(
                    "Prefetcher detected a reorg beyond acceptable depth. No hash for {}H",
                    self.cur_height
                );
            } else {
                let max_prev_hash = self
                    .prev_hashes
                    .iter()
                    .next_back()
                    .expect("At least one element");
                if self.cur_height != *max_prev_hash.0 + 1 {
                    for (h, hash) in self.prev_hashes.iter() {
                        debug!("prev_hash {}H -> {}", h, hash);
                    }
                    panic!(
                        "No prev_hash for a new block {}H {}; max_prev_hash: {}H {}",
                        self.cur_height, block.block.id, max_prev_hash.0, max_prev_hash.1
                    );
                }
            }
        }
        self.prev_hashes
            .insert(block.block.height, block.block.id.clone());
        // this is how big reorgs we're going to detect
        let window_size = 1000;
        if self.cur_height >= window_size {
            self.prev_hashes.remove(&(self.cur_height - window_size));
        }
        assert!(self.prev_hashes.len() <= window_size as usize);

        false
    }

    /// Handle condition detected by `detected_reorg`
    ///
    /// Basically, stop all workers (discarding their work), adjust height and
    /// start workers again.
    ///
    /// This doesn't have to be blazing fast, so it isn't.
    fn reset_on_reorg(&mut self) {
        debug!(
            "Resetting on reorg from {}H to {}H",
            self.cur_height,
            self.cur_height - 1
        );
        self.stop_workers();
        assert!(self.cur_height > 0);
        self.cur_height -= 1;
        self.start_workers();
    }
}

impl<R> Prefetcher<R>
where
    R: Rpc,
{
    fn stop_workers(&mut self) {
        self.workers_finish.store(true, Ordering::SeqCst);

        while let Ok(_) = self
            .rx
            .as_ref()
            .expect("start_workers called before stop_workers")
            .recv()
        {}

        self.rx = None;
        self.thread_joins.drain(..).map(|j| j.join()).for_each(drop);
        self.out_of_order_items.clear();
    }
}

impl<R> Iterator for Prefetcher<R>
where
    R: Rpc + 'static,
{
    type Item = RpcBlock<R>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.end_of_fast_sync == self.cur_height {
            debug!(
                "Prefetcher: end of fast sync at {}H; switching to one worker",
                self.cur_height
            );
            self.stop_workers();
            self.thread_num = 1;
            self.start_workers();
        }

        'retry_on_reorg: loop {
            if let Some(item) = self.out_of_order_items.remove(&self.cur_height) {
                if self.track_reorgs(&item) {
                    self.reset_on_reorg();
                    continue 'retry_on_reorg;
                }
                self.cur_height += 1;
                return Some(item.block);
            }

            loop {
                debug!(
                    "Waiting for the block from the workers at: {}H",
                    self.cur_height
                );
                let item = self
                    .rx
                    .as_ref()
                    .expect("rx available")
                    .recv()
                    .expect("Workers shouldn't disconnect");
                debug!(
                    "Got the block from the workers from: {}H",
                    item.block.height
                );
                if item.block.height == self.cur_height {
                    if self.track_reorgs(&item) {
                        self.reset_on_reorg();
                        continue 'retry_on_reorg;
                    }
                    self.cur_height += 1;
                    return Some(item.block);
                } else {
                    assert!(item.block.height > self.cur_height);
                    self.out_of_order_items.insert(item.block.height, item);
                }
            }
        }
    }
}

impl<R> Drop for Prefetcher<R>
where
    R: Rpc,
{
    fn drop(&mut self) {
        self.stop_workers();
    }
}

/// One worker thread, polling for data from the node
struct Worker<R>
where
    R: Rpc,
{
    rpc: Arc<R>,
    next_height: Arc<AtomicUsize>,
    workers_finish: Arc<AtomicBool>,
    tx: crossbeam_channel::Sender<RpcBlockWithPrevId<R>>,
    in_progress: Arc<Mutex<BTreeSet<BlockHeight>>>,
}

impl<R> Worker<R>
where
    R: Rpc,
{
    fn run(&mut self) {
        loop {
            let height = self.get_height_to_fetch();

            let mut retry_count = 0;
            'retry: loop {
                if self.workers_finish.load(Ordering::SeqCst) {
                    return;
                }

                match self.get_block_by_height(height) {
                    Err(e) => {
                        trace!("Error from the node: {}", e);
                        let ahead_minimum = height
                            - self
                                .get_min_height_in_progress()
                                .expect("at least current height");
                        std::thread::sleep(Duration::from_millis(
                            R::RECOMMENDED_ERROR_RETRY_DELAY_MS * ahead_minimum,
                        ));
                        retry_count += 1;
                        if retry_count % 10 == 0 {
                            debug!("Worker retrying rpc error {} at {}H", e, height);
                        }
                    }
                    Ok(None) => {
                        let sleep_ms = R::RECOMMENDED_HEAD_RETRY_DELAY_MS;
                        std::thread::sleep(Duration::from_millis(sleep_ms));
                    }
                    Ok(Some(item)) => {
                        self.tx.send(item).expect("Send must not fail");
                        self.mark_height_fetched(height);
                        break 'retry;
                    }
                }
            }
        }
    }

    fn get_height_to_fetch(&self) -> BlockHeight {
        let height = self.next_height.fetch_add(1, Ordering::SeqCst) as u64;
        self.in_progress
            .lock()
            .expect("unlock works")
            .insert(height);
        height
    }

    fn get_min_height_in_progress(&self) -> Option<BlockHeight> {
        let in_progress = self.in_progress.lock().expect("unlock works");
        in_progress.iter().next().cloned()
    }

    fn mark_height_fetched(&self, height: BlockHeight) {
        assert!(self
            .in_progress
            .lock()
            .expect("unlock works")
            .remove(&height));
    }

    fn get_block_by_height(
        &mut self,
        height: BlockHeight,
    ) -> Result<Option<RpcBlockWithPrevId<R>>> {
        if let Some(id) = self.rpc.get_block_id_by_height(height)? {
            Ok(self.rpc.get_block_by_id(&id)?.map(|block| {
                (RpcBlockWithPrevId {
                    block: Block {
                        height,
                        id,
                        data: block.0,
                    },
                    prev_block_id: block.1,
                })
            }))
        } else {
            Ok(None)
        }
    }
}
