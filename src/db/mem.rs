use super::*;
use common_failures::prelude::*;

#[derive(Default)]
pub struct MemDataStore {
    blocks: BTreeMap<BlockHeight, Block>,
    block_hashes: BTreeMap<BlockHeight, BlockHash>,
}

impl DataStore for MemDataStore {
    fn get_hash_by_height(&self, height: BlockHeight) -> Result<Option<BlockHash>> {
        Ok(self.block_hashes.get(&height).cloned())
    }
    /*
        fn get_chain_head(&self) -> Result<Option<(BlockHeight, BlockHash)>> {
            Ok(self
                .blocks
                .iter()
                .next_back()
                .map(|(k, v)| (*k, v.hash.clone())))
        }
    */
    fn reorg_at_height(&mut self, height: BlockHeight) -> Result<()> {
        for height in height.. {
            if self.blocks.remove(&height).is_none() {
                break;
            }
            self.block_hashes
                .remove(&height)
                .expect("block_hashes out of sync");
        }

        Ok(())
    }

    fn insert(&mut self, info: &BlockInfo) -> Result<()> {
        let (block, txs, utxo, spends) = super::parse_node_block(&info)?;
        self.blocks.insert(info.height, block);
        Ok(())
    }

    /*
    fn get_min_height(&self) -> Result<Option<BlockHeight>> {
        Ok(self.blocks.keys().next().cloned())
    }
    */

    fn get_max_height(&self) -> Result<Option<BlockHeight>> {
        Ok(self.blocks.keys().next_back().cloned())
    }
}
