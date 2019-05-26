use super::prefetcher;
use crate::Rpc;
use std::sync::Arc;

use crate::{WithHeightAndId, BlockHeight};
use common_failures::prelude::*;

/// Block fetcher
///
/// Fetcher builds on top of `Prefetcher``, addining ability
/// to start end  finish at a given height
pub struct Fetcher<R>
where
    R: Rpc,
{
    cached_node_block_count: Option<u32>,
    prefetcher: prefetcher::Prefetcher<R>,
    rpc: Arc<R>,
    ended: bool,
    end: Option<BlockHeight>,
}

impl<R> Fetcher<R>
where
    R: Rpc + Sync + 'static,
{
    pub fn new(rpc: Arc<R>, start: Option<WithHeightAndId<R::Id>>, end: Option<BlockHeight>) -> Result<Self>
    where
        R: Rpc,
    {
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

impl<R> Iterator for Fetcher<R>
where
    R: Rpc + 'static,
{
    type Item = WithHeightAndId<R::Id, R::Data>;

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
