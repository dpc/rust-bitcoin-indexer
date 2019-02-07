use super::*;
use log::{debug, info, trace};
use quickcheck_macros::quickcheck;
use std::sync::{Arc, Mutex};

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

#[derive(Copy, Clone, Debug)]
struct ReorgParams {
    delay: u8,
    depth: u8,
    add: u8,
}

impl ReorgParams {
    fn is_done(self) -> bool {
        self.delay == 0 && self.depth == 0 && self.add == 0
    }
}

struct TestRpcInner {
    reorgs: Vec<ReorgParams>,
    current_reorg: Option<ReorgParams>,
    chain: Vec<usize>,
    next_block_data: usize,
}

impl TestRpcInner {
    fn mine_block(&mut self) {
        self.chain.push(self.next_block_data);
        self.next_block_data += 1;
    }
}

struct TestRpc {
    inner: Mutex<TestRpcInner>,
}

impl TestRpc {
    fn new(start: Option<u8>, reorgs_base: Vec<(u8, u8, u8)>) -> Self {
        let reorgs: Vec<_> = reorgs_base
            .into_iter()
            .map(|n| ReorgParams {
                delay: n.0,
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

    fn maybe_change_state(&self) {
        let mut inner = self.inner.lock().unwrap();

        if inner.current_reorg.map(|c| c.is_done()).unwrap_or(false) {
            inner.current_reorg = None;
        }
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
                reorg.delay = 0;
            } else {
                trace!(
                    "Doing a reorg of depth {} and then adding {}",
                    reorg.depth,
                    reorg.add
                );
                let new_chain_len = inner.chain.len().saturating_sub(reorg.depth as usize);
                inner.chain.truncate(new_chain_len);
                for _ in 0..reorg.depth + reorg.add {
                    inner.mine_block()
                }
                reorg.depth = 0;
                reorg.add = 0;
            }
            inner.current_reorg = Some(reorg);
        } else {
            drop(inner);
            assert!(self.is_done());
        }
    }
}

const ID_HEIGHT_OFFSET: usize = 3;
impl Rpc for TestRpc {
    type Data = usize;
    type Id = usize;
    const RECOMMENDED_HEAD_RETRY_DELAY_MS: u64 = 0;

    fn get_block_count(&self) -> Result<u64> {
        self.maybe_change_state();

        let inner = self.inner.lock().unwrap();
        Ok(inner.chain.len().saturating_sub(1) as u64)
    }

    fn get_block_id_by_height(&self, height: prelude::BlockHeight) -> Result<Self::Id> {
        Ok(height as usize + ID_HEIGHT_OFFSET)
    }

    fn get_block_by_id(&self, hash: &Self::Id) -> Result<Option<(Self::Data, Self::Id)>> {
        let height = hash - ID_HEIGHT_OFFSET;
        let inner = self.inner.lock().unwrap();
        if inner.chain.len() <= height {
            drop(inner);
            self.maybe_change_state();
            return Ok(None);
        }
        let data = inner.chain[height as usize];
        Ok(Some((data, height + ID_HEIGHT_OFFSET - 1)))
    }
}

#[test]
fn prefetcher_reorg_reliability_fixed() {
    env_logger::init();
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

fn prefetcher_reorg_reliability(start: Option<u8>, reorgs_seed: Vec<(u8, u8, u8)>) -> bool {
    info!(
        "Prefetcher reliability; start {:?}H; reorgs_params.len() == {}",
        start,
        reorgs_seed.len()
    );
    debug!("reorgs_seed: {:?}", reorgs_seed);

    let rpc = Arc::new(TestRpc::new(start, reorgs_seed));
    let mut chain = rpc.get_current_chain();
    let pending_reorgs_on_start = rpc.get_current_pending_reorgs();
    debug!("starting chain: {:?}", chain);

    let start = start.map(|start| {
        let prefetcher_starting_height = u64::from(start.saturating_sub(16));
        let prefetcher_starting_id = rpc
            .get_block_id_by_height(prefetcher_starting_height)
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
        debug!("intern: {:?}", intern_chain);
        debug!("extern: {:?}", chain);
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
    println!("intern chain: {:?}", rpc_chain);
    println!("extern chain: {:?}", chain);
    if chain != rpc_chain {
        println!("Not equal!");
    }
    chain == rpc_chain
}
