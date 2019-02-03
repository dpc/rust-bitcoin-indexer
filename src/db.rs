pub mod mem;
pub mod pg;

use crate::prelude::*;
use common_failures::prelude::*;
use std::collections::BTreeMap;
use std::str::FromStr;

pub trait DataStore {
    /// Wipe the db
    fn wipe(&mut self) -> Result<()>;

    /// Fresh start
    fn mode_fresh(&mut self) -> Result<()>;

    /// We will be adding a lot stuff
    /// so go fast (eg. drop indices)
    fn mode_bulk(&mut self) -> Result<()>;

    /// We're at chain-head
    fn mode_normal(&mut self) -> Result<()>;

    fn wipe_to_height(&mut self, height: u64) -> Result<()>;

    fn get_max_height(&mut self) -> Result<Option<BlockHeight>>;
    fn get_hash_by_height(&mut self, height: BlockHeight) -> Result<Option<BlockHash>>;
    fn insert(&mut self, info: BlockInfo) -> Result<()>;
}

#[derive(Debug, Clone)]
struct Block {
    pub height: BlockHeight,
    pub hash: BlockHash,
    pub prev_hash: BlockHash,
    pub merkle_root: Sha256dHash,
    pub time: u32,
}

impl Block {
    pub fn from_core_block(info: &BlockInfo) -> Self {
        Block {
            height: info.height,
            hash: info.hash,
            prev_hash: info.block.header.prev_blockhash,
            merkle_root: info.block.header.merkle_root,
            time: info.block.header.time,
        }
    }
}
#[derive(Debug, Clone)]
struct Tx {
    pub hash: TxHash,
    pub block_hash: BlockHash,
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
            block_hash: info.hash,
            hash: tx_id,
            coinbase,
        }
    }
}
#[derive(Debug, Clone)]
struct Output {
    pub out_point: OutPoint,
    pub value: u64,
    pub address: Option<String>,
    pub coinbase: bool,
}

impl Output {
    pub fn from_core_block(
        _info: &BlockInfo,
        tx_id: TxHash,
        tx: &bitcoin_core::Transaction,
        idx: u32,
        tx_out: &bitcoin_core::TxOut,
    ) -> Self {
        let network = bitcoin::network::constants::Network::Bitcoin;
        Self {
            out_point: OutPoint {
                txid: tx_id,
                vout: idx,
            },
            value: tx_out.value,
            address: address_from_script(&tx_out.script_pubkey, network).map(|a| a.to_string()),
            coinbase: tx.is_coin_base(),
        }
    }
}

/// Created when Output is spent, referencing it
#[derive(Debug, Clone)]
struct Input {
    pub out_point: OutPoint,
    pub tx_id: TxHash,
}

impl Input {
    pub fn from_core_block(_info: &BlockInfo, tx_id: TxHash, tx_in: &bitcoin_core::TxIn) -> Self {
        Input {
            out_point: OutPoint {
                txid: tx_in.previous_output.txid,
                vout: tx_in.previous_output.vout,
            },
            tx_id,
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
            for (_idx, tx_in) in tx.input.iter().enumerate() {
                inputs.push(Input::from_core_block(info, tx_id, tx_in));
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
