use super::*;
use log::{debug, info, trace};
use quickcheck_macros::quickcheck;
use std::sync::{Arc, Mutex};

/// Insert new block (`height` + `data`) into a vector of blocks `chain`
///
/// This is to model using a `Vec` as a blockchain-datastructure:
///
/// * truncate the block if something is allready at `height` (reorg)
/// * refuse (panic) to insert something higher then next empty `height`
fn chain_insert(chain: &mut Vec<usize>, height: usize, data: usize) {
    if height < chain.len() {
        debug!("Truncating the chain from {} to {}", chain.len(), height);
        chain.truncate(height);
    } else if chain.len() > height {
        panic!(
            "Trying to insert {}H to chain of len {}",
            height,
            chain.len()
        );
    }
    assert_eq!(chain.len(), height);
    chain.push(data);
}

/// Paramters for a test reorg
#[derive(Copy, Clone, Debug)]
struct ReorgParams {
    /// Down-counter before the reorg is happening
    delay: u8,
    /// Num. of blocks to orphan
    depth: u8,
    /// Num of blocks to add on top of blocks replacing the orphaned ones
    add: u8,
}

/// Synchronized inner-data of a `TestRpc`
#[derive(Debug)]
struct TestRpcInner {
    /// Sequence of reorgs to perform
    reorgs: Vec<ReorgParams>,
    /// Currently pending reorg
    current_reorg: Option<ReorgParams>,
    /// State of the blockchain
    chain: Vec<usize>,
    /// Counter tracking unique data (and id derived from it)
    next_block_data: usize,
}

impl TestRpcInner {
    /// Simulate adding new block to a `chain`
    fn mine_block(&mut self) {
        self.chain.push(self.next_block_data);
        self.next_block_data += 1;
    }
}

/// An test Rpc implementation
///
/// Current implementation is kind of dumb and just does
/// reorgs. Lots of reorgs. But it's a good enough stresstest
/// and actually caught some corner cases already.
struct TestRpc {
    inner: Mutex<TestRpcInner>,
}

impl TestRpc {
    fn new(start: Option<u8>, reorgs_base: Vec<(u8, u8, u8)>) -> Self {
        let reorgs: Vec<_> = reorgs_base
            .into_iter()
            .map(|n| ReorgParams {
                delay: n.0 % 8,
                depth: n.1,
                add: n.2 + 1,
            })
            .collect();

        debug!("Starting with reorg sequence of len: {}", reorgs.len());

        let mut inner = TestRpcInner {
            reorgs,
            current_reorg: None,
            chain: vec![],
            next_block_data: 1337,
        };

        for _ in 0..start.map(|n| n + 1).unwrap_or(0) {
            inner.mine_block();
        }

        Self {
            inner: Mutex::new(inner),
        }
    }

    fn is_done(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.current_reorg.is_none() && inner.reorgs.is_empty()
    }

    fn get_current_chain(&self) -> Vec<usize> {
        let inner = self.inner.lock().unwrap();
        inner.chain.clone()
    }

    fn get_current_pending_reorgs(&self) -> Vec<ReorgParams> {
        let inner = self.inner.lock().unwrap();
        inner.reorgs.clone()
    }

    /// Triger state change
    ///
    /// This should be called from rpc handling functions
    /// periodically to simulate reorgs happening one by one.
    fn maybe_change_state(&self) {
        let mut inner = self.inner.lock().unwrap();

        debug!("Maybe change state; inner: {:?}", *inner);
        if inner.current_reorg.is_none() {
            inner.current_reorg = inner.reorgs.pop();
            if let Some(current_reorg) = inner.current_reorg {
                debug!("Moving to new reorg: {:?}", current_reorg);
            }
        }
        if inner.current_reorg.is_some() {
            let mut reorg = inner.current_reorg.unwrap();
            trace!("Current reorg: {:?}", reorg);

            if reorg.delay != 0 {
                trace!("Delay");
                reorg.delay -= 1;
                inner.current_reorg = Some(reorg);
            } else {
                debug!(
                    "Doing a reorg of depth {} and then adding {}",
                    reorg.depth, reorg.add
                );
                let new_chain_len = inner.chain.len().saturating_sub(reorg.depth as usize);
                inner.chain.truncate(new_chain_len);
                for _ in 0..(u64::from(reorg.depth) + u64::from(reorg.add)) {
                    inner.mine_block()
                }

                inner.current_reorg = None;
            }

            debug!("After maybe change state; inner: {:?}", *inner);
        } else {
            drop(inner);
            assert!(self.is_done());
        }
    }
}

/// To simplify the implementation every block's
/// data is just a fixed offset from it's id
const DATA_TO_ID_OFFSET: usize = 3;

impl Rpc for TestRpc {
    type Data = usize;
    type Id = usize;
    const RECOMMENDED_HEAD_RETRY_DELAY_MS: u64 = 0;

    fn get_block_count(&self) -> Result<u64> {
        self.maybe_change_state();

        let inner = self.inner.lock().unwrap();
        Ok(inner.chain.len().saturating_sub(1) as u64)
    }

    fn get_block_id_by_height(&self, height: prelude::BlockHeight) -> Result<Option<Self::Id>> {
        let inner = self.inner.lock().unwrap();
        let res = inner
            .chain
            .get(height as usize)
            .map(|data| data + DATA_TO_ID_OFFSET);

        if res.is_none() {
            drop(inner);
            self.maybe_change_state();
        }
        Ok(res)
    }

    fn get_block_by_id(&self, hash: &Self::Id) -> Result<Option<(Self::Data, Self::Id)>> {
        let needed_data = hash - DATA_TO_ID_OFFSET;
        let inner = self.inner.lock().unwrap();
        if let Some((height, data)) = inner
            .chain
            .iter()
            .enumerate()
            .find(|(_height, data)| **data == needed_data)
        {
            let prev_data = inner.chain[height.saturating_sub(1)];
            Ok(Some((*data, prev_data + DATA_TO_ID_OFFSET)))
        } else {
            drop(inner);
            self.maybe_change_state();
            Ok(None)
        }
    }
}

#[test]
/// Some test cases known to trigger issues in the past
fn prefetcher_reorg_reliability_fixed() {
    for (start, reorgs_seed) in vec![
        (None, vec![(0, 1, 0), (0, 0, 0), (40, 69, 70)]),
        (None, vec![(0, 1, 0), (1, 56, 84)]),
    ] {
        assert!(prefetcher_reorg_reliability(start, reorgs_seed));
    }
}

#[quickcheck]
fn prefetcher_reorg_reliability_quickcheck(
    start: Option<u8>,
    reorgs_seed: Vec<(u8, u8, u8)>,
) -> bool {
    prefetcher_reorg_reliability(start, reorgs_seed)
}

fn prefetcher_reorg_reliability(start: Option<u8>, mut reorgs_seed: Vec<(u8, u8, u8)>) -> bool {
    info!(
        "Prefetcher reliability; start {:?}H; reorgs_params.len() == {}",
        start,
        reorgs_seed.len()
    );
    reorgs_seed.truncate(32);

    debug!("reorgs_seed: {:?}", reorgs_seed);

    let rpc = Arc::new(TestRpc::new(start, reorgs_seed));
    let mut chain = rpc.get_current_chain();
    let pending_reorgs_on_start = rpc.get_current_pending_reorgs();

    let window_size = pending_reorgs_on_start
        .iter()
        .fold(0u64, |sum, reorg| sum + u64::from(reorg.depth));

    debug!("starting chain: {:?}", chain);

    let start = start.map(|start| {
        let prefetcher_starting_height = u64::from(start).saturating_sub(window_size);
        let prefetcher_starting_id = rpc
            .get_block_id_by_height(prefetcher_starting_height)
            .unwrap()
            .unwrap();

        Block {
            height: prefetcher_starting_height,
            hash: prefetcher_starting_id,
            data: (),
        }
    });

    let mut prefetcher = prefetcher::Prefetcher::new(rpc.clone(), start).unwrap();

    loop {
        let intern_chain = rpc.get_current_chain();
        trace!("intern: {:?}", intern_chain);
        trace!("extern: {:?}", chain);
        if rpc.is_done() {
            // needs to be re-read after `is_done` is checked
            let intern_chain = rpc.get_current_chain();
            if intern_chain.len() == chain.len() {
                break;
            } else {
                debug!(
                    "TestRPC is done; waiting for pending blocks; extern {} vs intern {}",
                    chain.len(),
                    intern_chain.len()
                );
                debug!("original reorg pattern: {:?}", pending_reorgs_on_start);
            }
        }

        let item = prefetcher.next().unwrap();
        debug!(
            "prefetcher returned: {}H (id: {}; data: {})",
            item.height, item.hash, item.data
        );
        chain_insert(&mut chain, item.height as usize, item.data);
    }

    let rpc_chain = rpc.get_current_chain();
    chain == rpc_chain
}
