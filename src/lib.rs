#![feature(uniform_paths)]

pub mod db;
pub mod fetcher;
pub mod opts;
pub mod prefetcher;
pub mod prelude;
pub mod utils;

pub use prelude::{Block, BlockCore};

use common_failures::prelude::*;
use std::fmt::{Debug, Display};

/// `Block` specialized over types from `Rpc`
type RpcBlock<R> = Block<<R as Rpc>::Id, <R as Rpc>::Data>;

/// `RpcBlock` along with an Id of a previous block
struct RpcBlockWithPrevId<R: Rpc> {
    block: RpcBlock<R>,
    prev_block_id: R::Id,
}

/// An minimum interface for node rpc that prefetcher can work with
pub trait Rpc: Send + Sync {
    type Data: Send;
    type Id: Send + Eq + PartialEq + Display + Debug + Clone;
    const RECOMMENDED_HEAD_RETRY_DELAY_MS: u64;

    fn get_block_count(&self) -> Result<u64>;

    fn get_block_id_by_height(&self, height: prelude::BlockHeight) -> Result<Option<Self::Id>>;

    /// Get the block by id, along with id of the previous block
    fn get_block_by_id(&self, hash: &Self::Id) -> Result<Option<(Self::Data, Self::Id)>>;
}

#[cfg(test)]
mod tests;
