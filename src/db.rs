pub mod mem;
pub mod pg;

use crate::prelude::*;
use bitcoincore_rpc::Client;
use common_failures::prelude::*;
use std::collections::BTreeMap;
use std::str::FromStr;

pub trait DataStore {
    /// Initi the db schema etc.
    fn init(&mut self) -> Result<()>;

    /// Initi the db schema etc.
    fn wipe(&mut self) -> Result<()>;

    fn get_max_height(&mut self) -> Result<Option<BlockHeight>>;
    fn get_hash_by_height(&mut self, height: BlockHeight) -> Result<Option<BlockHash>>;
    fn reorg_at_height(&mut self, height: BlockHeight) -> Result<()>;
    fn insert(&mut self, info: BlockInfo) -> Result<()>;
    fn flush(&mut self) -> Result<()>;
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
    pub coinbase: bool,
}

impl Tx {
    pub fn from_core_block(
        info: &BlockInfo,
        tx_id: TxHash,
        tx: &bitcoin_core::Transaction,
    ) -> Self {
        let coinbase = tx.is_coin_base();
        Self {
            height: info.height,
            hash: tx_id,
            coinbase,
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
    pub coinbase: bool,
}

impl Output {
    pub fn from_core_block(
        info: &BlockInfo,
        tx_id: TxHash,
        tx: &bitcoin_core::Transaction,
        idx: u32,
        tx_out: &bitcoin_core::TxOut,
    ) -> Self {
        let network = bitcoin::network::constants::Network::Bitcoin;
        Self {
            height: info.height,
            tx_hash: tx_id,
            tx_idx: idx,
            value: tx_out.value,
            address: address_from_script(&tx_out.script_pubkey, network).map(|a| a.to_string()),
            coinbase: tx.is_coin_base(),
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

struct Parsed {
    pub block: Block,
    pub txs: Vec<Tx>,
    pub outputs: Vec<Output>,
    pub inputs: Vec<Input>,
}

fn parse_node_block(info: &BlockInfo) -> Result<Parsed> {
    let mut outputs: Vec<Output> = vec![];
    let mut inputs: Vec<Input> = vec![];
    let mut txs: Vec<Tx> = vec![];
    let block = Block::from_core_block(info);

    for tx in &info.block.txdata {
        let coinbase = tx.is_coin_base();

        let tx_id = if block.height == 91842 && coinbase {
            // d5d27987d2a3dfc724e359870c6644b40e497bdc0589a033220fe15429d88599
            // e3bf3d07d4b0375638d5f1db5255fe07ba2c4cb067cd81b84ee974b6585fb469
            //
            // are twice in the blockchain; eg.
            // https://blockchair.com/bitcoin/block/91812
            // https://blockchair.com/bitcoin/block/91842
            // to make the unique indexes happy, we just add one to last byte

            TxHash::from_str("d5d27987d2a3dfc724e359870c6644b40e497bdc0589a033220fe15429d885a0")
                .unwrap()
        } else if info.height == 91880 && coinbase {
            TxHash::from_str("e3bf3d07d4b0375638d5f1db5255fe07ba2c4cb067cd81b84ee974b6585fb469")
                .unwrap()
        } else {
            tx.txid()
        };
        txs.push(Tx::from_core_block(info, tx_id, &tx));
        for (idx, tx_out) in tx.output.iter().enumerate() {
            outputs.push(Output::from_core_block(
                info, tx_id, &tx, idx as u32, tx_out,
            ))
        }
        if !tx.is_coin_base() {
            for (idx, tx_in) in tx.input.iter().enumerate() {
                inputs.push(Input::from_core_block(info, &tx, idx as u32, tx_in));
            }
        }
    }

    let parsed = Parsed {
        block,
        txs,
        outputs,
        inputs,
    };
    Ok(parsed)
}
