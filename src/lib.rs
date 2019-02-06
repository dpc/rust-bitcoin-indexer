pub mod db;
pub mod fetcher;
pub mod opts;
pub mod prefetcher;
pub mod prelude;
pub mod utils;

pub use prelude::{Block, BlockCore};

use common_failures::prelude::*;
use std::fmt::Display;

/// An minimum interface for node rpc that prefetcher can work with
pub trait Rpc: Send + Sync {
    type Data: Send;
    type Id: Send + Eq + PartialEq + Display + Clone;
    fn get_block_count(&self) -> Result<u64>;
    fn get_block_id_by_height(&self, height: prelude::BlockHeight) -> Result<Self::Id>;
    fn get_block_by_id(&self, hash: &Self::Id) -> Result<Self::Data>;
}
