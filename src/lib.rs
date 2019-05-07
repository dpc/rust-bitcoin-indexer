pub mod db;
pub mod event_source;
pub mod node;
pub mod opts;
pub mod prelude;
pub mod util;
use prelude::*;

pub mod types;
pub use types::*;
pub use crate::{Block, BlockCore, BlockHeight};

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

    fn get_block_id_by_height(&self, height: BlockHeight) -> Result<Option<Self::Id>>;

    /// Get the block by id, along with id of the previous block
    fn get_block_by_id(&self, hash: &Self::Id) -> Result<Option<(Self::Data, Self::Id)>>;
}

#[derive(Clone, Debug)]
pub struct RpcInfo {
    pub url: String,
    pub auth: bitcoincore_rpc::Auth,
}

impl RpcInfo {
    pub fn new(url: String, user: Option<String>, pass: Option<String>) -> Result<Self> {
        let auth = match (user, pass) {
            (Some(u), Some(p)) => bitcoincore_rpc::Auth::UserPass(u.clone(), p.clone()),
            (None, None) => bitcoincore_rpc::Auth::None,
            _ => bail!("Incorrect node auth parameters"),
        };
        Ok(Self {url, auth })
    }
    pub fn to_rpc_client(&self) -> Result<bitcoincore_rpc::Client> {
        Ok(bitcoincore_rpc::Client::new(self.url.clone(), self.auth.clone())?)
    }
}
#[cfg(test)]
mod tests;
