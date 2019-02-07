use log::{debug, info, trace};

use crate::prelude::*;
use crate::Rpc;
use common_failures::prelude::*;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{
        self,
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
    type Data = bitcoin::Block;
    type Id = bitcoin::util::hash::Sha256dHash;
    const RECOMMENDED_HEAD_RETRY_DELAY_MS: u64 = 1000;

    fn get_block_count(&self) -> Result<u64> {
        Ok(self.get_block_count()?)
    }

    fn get_block_id_by_height(&self, height: BlockHeight) -> Result<Self::Id> {
        Ok(self.get_block_hash(height)?)
    }

    fn get_block_by_id(&self, hash: &Self::Id) -> Result<Option<(Self::Data, Self::Id)>> {
        let block: bitcoin::Block = match self.get_by_id(hash) {
            Err(e) => {
                if e.to_string().contains("Block height out of range") {
                    return Ok(None);
                } else {
                    return Err(e.into());
                }
            }
            Ok(o) => o,
        };
        let prev_id = block.header.prev_blockhash;
        Ok(Some((block, prev_id)))
    }
}

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
    rx: Option<crossbeam_channel::Receiver<(Block<R::Data, R::Id>, R::Id)>>,
    /// Worker threads
    thread_joins: Vec<std::thread::JoinHandle<()>>,
    /// List of blocks that arrived out-of-order: before the block
    /// we were actually waiting for.
    out_of_order_items: HashMap<BlockHeight, (Block<R::Data, R::Id>, R::Id)>,

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
    pub fn new(rpc: Arc<R>, last_block: Option<Block<(), R::Id>>) -> Result<Self> {
        let thread_num = num_cpus::get() * 2;
        let workers_finish = Arc::new(AtomicBool::new(false));

        let end_of_fast_sync = retry(|| Ok(rpc.get_block_count()?));
        let mut prev_hashes = BTreeMap::default();
        let start = if let Some(h_and_hash) = last_block {
            let h = h_and_hash.height;
            prev_hashes.insert(h, h_and_hash.hash);
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

        let (tx, rx) = crossbeam_channel::bounded(self.thread_num * 8);
        self.rx = Some(rx);
        let retry_set = Arc::new(sync::Mutex::new(default()));
        let next_height = Arc::new(AtomicUsize::new(self.cur_height as usize));
        assert!(self.thread_joins.is_empty());
        for _ in 0..self.thread_num {
            self.thread_joins.push({
                std::thread::spawn({
                    let next_height = next_height.clone();
                    let rpc = self.rpc.clone();
                    let tx = tx.clone();
                    let workers_finish = self.workers_finish.clone();
                    let retry_set = retry_set.clone();
                    move || {
                        // TODO: constructor
                        let mut worker = Worker {
                            retry_set,
                            next_height,
                            workers_finish,
                            rpc,
                            tx,
                            retry_count: 0,
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
    fn track_reorgs(&mut self, current: &Block<R::Data, R::Id>, prev_id: R::Id) -> bool {
        debug_assert_eq!(current.height, self.cur_height);
        if self.cur_height > 0 {
            if let Some(stored_prev_id) = self.prev_hashes.get(&(self.cur_height - 1)) {
                if stored_prev_id != &prev_id {
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
                    panic!(
                        "No prev_hash for a new block {}H {}; max_prev_hash: {}H {}",
                        self.cur_height, current.hash, max_prev_hash.0, max_prev_hash.1
                    );
                }
            }
        }
        self.prev_hashes
            .insert(current.height, current.hash.clone());
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
    type Item = Block<R::Data, R::Id>;
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
                if self.track_reorgs(&item.0, item.1) {
                    self.reset_on_reorg();
                    continue 'retry_on_reorg;
                }
                self.cur_height += 1;
                return Some(item.0);
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
                debug!("Got the block from the workers from: {}H", item.0.height);
                if item.0.height == self.cur_height {
                    if self.track_reorgs(&item.0, item.1) {
                        self.reset_on_reorg();
                        continue 'retry_on_reorg;
                    }
                    self.cur_height += 1;
                    return Some(item.0);
                } else {
                    assert!(item.0.height > self.cur_height);
                    self.out_of_order_items.insert(item.0.height, item);
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
    retry_set: Arc<Mutex<BTreeSet<BlockHeight>>>,
    workers_finish: Arc<AtomicBool>,
    tx: crossbeam_channel::Sender<(Block<R::Data, R::Id>, R::Id)>,
    retry_count: usize,
}

impl<R> Worker<R>
where
    R: Rpc,
{
    fn run(&mut self) {
        while !self.workers_finish.load(Ordering::SeqCst) {
            let height = self
                .get_from_retry_set()
                .unwrap_or_else(|| self.get_from_next_height());

            match self.get_block_by_height(height) {
                Err(e) => {
                    self.insert_into_retry_set(height);
                    trace!("Error from the node: {}", e);
                    std::thread::sleep(Duration::from_millis(self.retry_count as u64 * 1000 + 100));
                    self.retry_count += 1;
                    if self.retry_count % 5 == 0 {
                        eprintln!("{} (retrying...)", e.display_causes_and_backtrace());

                        debug!("DELME; ERROR {} at {}", e, height,);
                    }
                }
                Ok(None) => {
                    self.insert_into_retry_set(height);
                    let sleep_ms = R::RECOMMENDED_HEAD_RETRY_DELAY_MS;
                    self.retry_count += 1;
                    if self.retry_count >= 10000 {
                        debug!("DELME; Ahead of the chain head at {}", height);
                        std::thread::sleep(Duration::from_millis(sleep_ms * 10000));
                    }
                    std::thread::sleep(Duration::from_millis(sleep_ms));
                }
                Ok(Some(item)) => {
                    self.retry_count = 0;
                    self.tx.send(item).expect("Send must not fail");
                }
            }
        }
    }

    fn get_from_retry_set(&self) -> Option<BlockHeight> {
        let mut set = self.retry_set.lock().unwrap();
        if let Some(height) = set.iter().next().cloned() {
            set.remove(&height);
            return Some(height);
        }

        None
    }

    fn insert_into_retry_set(&self, height: BlockHeight) {
        let mut set = self.retry_set.lock().unwrap();
        let not_present = set.insert(height);
        assert!(not_present);
    }

    fn get_from_next_height(&self) -> BlockHeight {
        self.next_height.fetch_add(1, Ordering::SeqCst) as u64
    }

    fn get_block_by_height(
        &mut self,
        height: BlockHeight,
    ) -> Result<Option<(Block<R::Data, R::Id>, R::Id)>> {
        let hash = self.rpc.get_block_id_by_height(height)?;
        Ok(self.rpc.get_block_by_id(&hash)?.map(|block| {
            (
                Block {
                    height,
                    hash,
                    data: block.0,
                },
                block.1,
            )
        }))
    }
}
