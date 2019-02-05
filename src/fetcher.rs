use crate::{prefetcher, prelude::*};
use std::sync::Arc;

use common_failures::prelude::*;

/// Block fetcher
///
/// Fetcher builds on top of `Prefetcher``, addining ability
/// to start end  finish at a given height
pub struct Fetcher {
    cached_node_block_count: Option<u64>,
    prefetcher: prefetcher::Prefetcher,
    rpc: Arc<bitcoincore_rpc::Client>,
    ended: bool,
    end: Option<BlockHeight>,
}

impl Fetcher {
    pub fn new(
        rpc: Arc<bitcoincore_rpc::Client>,
        start: Option<BlockHeightAndHash>,
        end: Option<BlockHeight>,
    ) -> Result<Self> {
        let prefetcher = prefetcher::Prefetcher::new(rpc.clone(), start)?;

        Ok(Fetcher {
            rpc,
            prefetcher,
            cached_node_block_count: None,
            ended: false,
            end,
        })
    }
}

impl Iterator for Fetcher {
    type Item = BlockCore;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }

        let item = self.prefetcher.next().expect("Prefetcher never ends");
        let height = item.height;

        if let Some(end) = self.end {
            if height >= end {
                self.ended = true;
            }
        } else if height > self.cached_node_block_count.unwrap_or(0) {
            loop {
                match self.rpc.get_block_count() {
                    Ok(block_count) => {
                        self.cached_node_block_count = Some(block_count);

                        if height >= block_count {
                            self.ended = true;
                        }
                        break;
                    }
                    Err(_e) => {
                        eprintln!("Retrying `getblockcount`");
                    }
                }
            }
        }

        Some(item)
    }
}
