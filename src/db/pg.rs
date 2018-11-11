use super::*;

struct Postresql;

impl DataStore for Postresql {
    fn get_max_height(&self) -> Result<Option<BlockHeight>> {
        unimplemented!();
    }
    fn get_hash_by_height(&self, height: BlockHeight) -> Result<Option<BlockHash>> {
        unimplemented!();
    }
    fn reorg_at_height(&mut self, height: BlockHeight) -> Result<()> {
        unimplemented!();
    }
    fn insert(&mut self, info: &BlockInfo) -> Result<()> {
        unimplemented!();
    }
}
