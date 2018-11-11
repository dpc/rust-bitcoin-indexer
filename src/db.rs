pub mod mem;
pub mod pg;

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
struct Output {
    pub height: BlockHeight,
    pub tx_hash: TxHash,
    pub tx_idx: u32,
    pub value: u64,
    pub address: Option<String>,
}

impl Output {
    pub fn from_core_block(
        info: &BlockInfo,
        tx: &bitcoin_core::Transaction,
        idx: u32,
        tx_out: &bitcoin_core::TxOut,
    ) -> Self {
        let network = bitcoin::network::constants::Network::Bitcoin;
        Self {
            height: info.height,
            tx_hash: tx.txid(),
            tx_idx: idx,
            value: tx_out.value,
            address: address_from_script(&tx_out.script_pubkey, network).map(|a| a.to_string()),
        }
    }
}

/// Created when Output is spent, referencing it
#[derive(Debug, Clone)]
struct Input {
    pub height: BlockHeight,
    pub utxo_tx_hash: TxHash,
    pub utxo_tx_idx: u32,
}

impl Input {
    pub fn from_core_block(
        info: &BlockInfo,
        _tx: &bitcoin_core::Transaction,
        idx: u32,
        tx_in: &bitcoin_core::TxIn,
    ) -> Self {
        Input {
            height: info.height,
            utxo_tx_hash: tx_in.previous_output.txid,
            utxo_tx_idx: tx_in.previous_output.vout,
        }
    }
}

fn parse_node_block(info: &BlockInfo) -> Result<(Block, Vec<Tx>, Vec<Output>, Vec<Input>)> {
    let mut outputs: Vec<Output> = vec![];
    let mut inputs: Vec<Input> = vec![];
    let mut txs: Vec<Tx> = vec![];
    let block = Block::from_core_block(info);

    for tx in &info.block.txdata {
        txs.push(Tx::from_core_block(info, &tx));
        for (idx, tx_out) in tx.output.iter().enumerate() {
            outputs.push(Output::from_core_block(info, &tx, idx as u32, tx_out))
        }
        for (idx, tx_in) in tx.input.iter().enumerate() {
            inputs.push(Input::from_core_block(info, &tx, idx as u32, tx_in));
        }
    }

    Ok((block, txs, outputs, inputs))
}
