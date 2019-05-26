pub mod mem;
pub mod pg;

use crate::{prelude::*, types::*, TxHash};
use std::collections::BTreeMap;

pub trait IndexerStore {
    /// Get the height of the stored chainhead
    fn get_head_height(&mut self) -> Result<Option<BlockHeight>>;

    /// Get hash of the stored block by height
    ///
    /// This will ignore any orphans
    fn get_hash_by_height(&mut self, height: BlockHeight) -> Result<Option<BlockHash>>;

    /// Insert a new block: either extending current `head` or starting a reorg
    fn insert(&mut self, info: crate::BlockData) -> Result<()>;
}

pub trait MempoolStore {
    fn insert_iter<'a>(
        &mut self,
        tx: impl Iterator<Item = &'a WithHash<Option<bitcoin::Transaction>>>,
    ) -> Result<()>;
    fn insert(&mut self, tx: &WithHash<Option<bitcoin::Transaction>>) -> Result<()>;
}
