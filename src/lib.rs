pub mod db;
pub mod event_source;
pub mod node;
pub mod opts;
pub mod prelude;
pub mod util;
use prelude::*;

pub mod types;
pub use crate::{WithHeightAndId, BlockData, BlockHeight};
pub use types::*;

use std::fmt::{Debug, Display};

/// `Block` specialized over types from `Rpc`
type RpcBlock<R> = WithHeightAndId<<R as Rpc>::Id, <R as Rpc>::Data>;

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
    const RECOMMENDED_ERROR_RETRY_DELAY_MS: u64;

    fn get_block_count(&self) -> Result<BlockHeight>;

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
    pub fn from_url(url_str: &str) -> Result<Self> {
        let mut url = url::Url::parse(url_str)?;
        let auth = match (url.username() == "", url.password()) {
            (false, Some(p)) => {
                bitcoincore_rpc::Auth::UserPass(url.username().to_owned(), p.to_owned())
            }
            (true, None) => bitcoincore_rpc::Auth::None,
            _ => bail!("Incorrect node auth parameters"),
        };
        url.set_password(None).expect("url lib sane");
        url.set_username("").expect("url lib sane");

        Ok(Self {
            url: url.to_string(),
            auth,
        })
    }
    pub fn to_rpc_client(&self) -> Result<bitcoincore_rpc::Client> {
        Ok(bitcoincore_rpc::Client::new(
            self.url.clone(),
            self.auth.clone(),
        )?)
    }
}
#[cfg(test)]
mod tests;
