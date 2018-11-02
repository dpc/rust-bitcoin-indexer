use bitcoin::util::hash::Sha256dHash;
use bitcoin_rpc::BitcoinRpc;
use common_failures::prelude::*;
use std::collections::BTreeMap;

pub mod bitcoin_core {
    pub use bitcoin::{
        blockdata::{
            block::Block,
            transaction::{Transaction, TxOut},
        },
        network::{
            encodable::ConsensusDecodable,
            serialize::{deserialize, serialize_hex, RawDecoder},
        },
        util::{
            hash::{Hash160, Sha256dHash},
            privkey::Privkey,
        },
    };
}

pub type BlockHeight = u64;
pub type BlockHash = Sha256dHash;

pub trait DataStore {
    fn get_chain_head(&self) -> Result<Option<(BlockHeight, BlockHash)>>;
    fn revert_head(&mut self) -> Result<()>;
    fn insert(&mut self, height: u64, block: Block) -> Result<()>;
}

pub struct Block {
    pub hash: BlockHash,
    pub height: BlockHeight,
    pub prev_hash: BlockHash,
}

impl Block {
    pub fn from_hex(hash: Sha256dHash, height: u64, hex: &str) -> Result<Self> {
        let bytes = hex::decode(hex)?;
        let block: bitcoin_core::Block = bitcoin_core::deserialize(&bytes)?;
        Ok(Block {
            hash,
            height,
            prev_hash: block.header.prev_blockhash,
        })
    }

    pub fn fetch_by_height(rpc: &BitcoinRpc, height: u64) -> Result<Self> {
        let block_hash = rpc.get_blockhash(height)?;
        let block_hex = rpc.get_block(&block_hash)?;
        Block::from_hex(block_hash, height, &block_hex)
    }
}
#[derive(Default)]
pub struct MemDataStore {
    blocks: BTreeMap<BlockHeight, Block>,
}

impl MemDataStore {
    pub fn new() -> Self {
        Default::default()
    }
}

impl DataStore for MemDataStore {
    fn get_chain_head(&self) -> Result<Option<(BlockHeight, BlockHash)>> {
        Ok(self
            .blocks
            .iter()
            .next_back()
            .map(|(k, v)| (*k, v.hash.clone())))
    }

    fn revert_head(&mut self) -> Result<()> {
        let top = self.blocks.keys().next_back().unwrap().clone();
        self.blocks.remove(&top);

        Ok(())
    }

    fn insert(&mut self, height: u64, block: Block) -> Result<()> {
        self.blocks.insert(height, block);
        Ok(())
    }
}
