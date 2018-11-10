pub mod mem;

use bitcoincore_rpc::Client;
use common_failures::prelude::*;
use crate::prelude::*;
use std::collections::BTreeMap;

pub trait DataStore {
    fn get_max_height(&self) -> Result<Option<BlockHeight>>;
    fn get_hash_by_height(&self, height: BlockHeight) -> Result<Option<BlockHash>>;
    fn reorg_at_height(&mut self, height: BlockHeight) -> Result<()>;
    fn insert(&mut self, info: &BlockInfo) -> Result<()>;
}

#[derive(Debug, Clone)]
struct Block {
    pub height: BlockHeight,
    pub hash: BlockHash,
    pub prev_hash: BlockHash,
}

impl Block {
    pub fn from_core_block(info: &BlockInfo) -> Self {
        Block {
            height: info.height,
            hash: info.hash,
            prev_hash: info.block.header.prev_blockhash,
        }
    }
}
#[derive(Debug, Clone)]
struct Tx {
    pub height: BlockHeight,
    pub hash: TxHash,
}

impl Tx {
    pub fn from_core_block(info: &BlockInfo, tx: &bitcoin_core::Transaction) -> Self {
        Self {
            height: info.height,
            hash: tx.txid(),
        }
    }
}
#[derive(Debug, Clone)]
struct Utxo {
    pub height: BlockHeight,
    pub tx: TxHash,
    pub idx: u16,
    pub value: u64,
}

impl Utxo {
    pub fn from_core_block(
        info: &BlockInfo,
        tx: &bitcoin_core::Transaction,
        idx: u16,
        tx_out: &bitcoin_core::TxOut,
    ) -> Self {
        Self {
            height: info.height,
            tx: tx.txid(),
            idx,
            value: tx_out.value,
        }
    }
}
/// Created when Utxo is spent, referencing it
#[derive(Debug, Clone)]
struct Spend {
    pub height: BlockHeight,
    pub tx: TxHash,
    pub idx: u16,
}

fn parse_node_block(info: &BlockInfo) -> Result<()> {
    let mut utxos: Vec<Utxo> = vec![];
    let mut spends: Vec<Spend> = vec![];
    let mut txs: Vec<Tx> = vec![];
    let block = Block::from_core_block(info);

    for tx in &info.block.txdata {
        txs.push(Tx::from_core_block(info, &tx));
        for (idx, tx_out) in tx.output.iter().enumerate() {
            utxos.push(Utxo::from_core_block(info, &tx, idx as u16, tx_out))
        }
    }
    unimplemented!();
}
