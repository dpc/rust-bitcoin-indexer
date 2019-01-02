use super::*;
use common_failures::prelude::*;

#[derive(Default)]
pub struct MemDataStore {
    blocks: BTreeMap<BlockHeight, Block>,
    block_hashes: BTreeMap<BlockHeight, BlockHash>,
}

impl DataStore for MemDataStore {
    fn init(&mut self) -> Result<()> {
        Ok(())
    }

    fn wipe(&mut self) -> Result<()> {
        Ok(())
    }

    fn mode_bulk(&mut self) -> Result<()> {
        Ok(())
    }

    fn mode_normal(&mut self) -> Result<()> {
        Ok(())
    }

    fn get_hash_by_height(&mut self, height: BlockHeight) -> Result<Option<BlockHash>> {
        Ok(self.block_hashes.get(&height).cloned())
    }

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

    fn insert(&mut self, info: BlockInfo) -> Result<()> {
        let parsed = super::parse_node_block(&info)?;
        self.blocks.insert(info.height, parsed.block);
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    fn get_max_height(&mut self) -> Result<Option<BlockHeight>> {
        Ok(self.blocks.keys().next_back().cloned())
    }
}
