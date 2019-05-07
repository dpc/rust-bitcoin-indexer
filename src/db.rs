pub mod mem;
pub mod pg;

use crate::{prelude::*, types::*, TxHash};
use std::collections::BTreeMap;

pub trait DataStore {
    fn wipe_to_height(&mut self, height: u64) -> Result<()>;

    /// Get the height of the stored chainhead
    fn get_head_height(&mut self) -> Result<Option<BlockHeight>>;

    /// Get hash of the stored block by height
    ///
    /// This will ignore any orphans
    fn get_hash_by_height(&mut self, height: BlockHeight) -> Result<Option<BlockHash>>;

    /// Insert a new block: either extending current `head` or starting a reorg
    fn insert(&mut self, info: crate::BlockCore) -> Result<()>;
}

/// Block data to be stored in the db
#[derive(Debug, Clone)]
struct Block {
    pub height: BlockHeight,
    pub hash: BlockHash,
    pub prev_hash: BlockHash,
    pub merkle_root: crate::Sha256dHash,
    pub time: u32,
}

impl Block {
    pub fn from_core_block(block: &crate::BlockCore) -> Self {
        Block {
            height: block.height,
            hash: block.id,
            prev_hash: block.data.header.prev_blockhash,
            merkle_root: block.data.header.merkle_root,
            time: block.data.header.time,
        }
    }
}

/// Tx data to be stored in the db
#[derive(Debug, Clone)]
struct Tx {
    pub hash: TxHash,
    pub block_hash: BlockHash,
    pub coinbase: bool,
    pub output_value_sum: u64,
    pub inputs: Vec<OutPoint>,
    pub weight: u64,
}

impl Tx {
    pub fn from_core_block(
        info: &crate::BlockCore,
        tx_id: TxHash,
        tx: &crate::Transaction,
    ) -> Self {
        let coinbase = tx.is_coin_base();
        Self {
            block_hash: info.id,
            hash: tx_id,
            coinbase,
            inputs: tx.input.iter().map(|tx_in| tx_in.previous_output).collect(),
            output_value_sum: tx.output.iter().fold(0, |acc, o| acc + o.value),
            weight: tx.get_weight(),
        }
    }
}

/// Output data to be stored in the db
#[derive(Debug, Clone)]
struct Output {
    pub out_point: OutPoint,
    pub value: u64,
    pub address: Option<String>,
    pub coinbase: bool,
}

impl Output {
    pub fn from_core_block(
        _info: &crate::BlockCore,
        tx_id: TxHash,
        tx: &crate::Transaction,
        idx: u32,
        tx_out: &crate::TxOut,
    ) -> Self {
        let network = bitcoin::network::constants::Network::Bitcoin;
        Self {
            out_point: OutPoint {
                txid: tx_id,
                vout: idx,
            },
            value: tx_out.value,
            address: crate::util::bitcoin::address_from_script(&tx_out.script_pubkey, network)
                .map(|a| a.to_string()),
            coinbase: tx.is_coin_base(),
        }
    }
}

/// Input to be stored in the db
///
/// Created when Output is spent, referencing spent output by `out_point`
#[derive(Debug, Clone)]
struct Input {
    pub out_point: OutPoint,
    /// Tx which includes the input (not to be mistaken by the `tx` which created the `out_point`
    pub tx_id: TxHash,
}

impl Input {
    pub fn from_core_block(_info: &crate::BlockCore, tx_id: TxHash, tx_in: &crate::TxIn) -> Self {
        Input {
            out_point: OutPoint {
                txid: tx_in.previous_output.txid,
                vout: tx_in.previous_output.vout,
            },
            tx_id,
        }
    }
}

/// All data from the block, parsed to types reflecting what the db
/// is actually storing.
struct Parsed {
    pub block: Block,
    pub txs: Vec<Tx>,
    pub outputs: Vec<Output>,
    pub inputs: Vec<Input>,
}

fn parse_node_block(block_core: &crate::BlockCore) -> Result<Parsed> {
    let mut outputs: Vec<Output> = vec![];
    let mut inputs: Vec<Input> = vec![];
    let mut txs: Vec<Tx> = vec![];
    let block = Block::from_core_block(block_core);

    for tx in &block_core.data.txdata {
        let coinbase = tx.is_coin_base();

        let tx_id = if block.height == 91842 && coinbase {
            // d5d27987d2a3dfc724e359870c6644b40e497bdc0589a033220fe15429d88599
            // e3bf3d07d4b0375638d5f1db5255fe07ba2c4cb067cd81b84ee974b6585fb469
            //
            // are twice in the blockchain; eg.
            // https://blockchair.com/bitcoin/block/91812
            // https://blockchair.com/bitcoin/block/91842
            // to make the unique indexes happy, we just add one to last byte

            TxHash::from_hex("d5d27987d2a3dfc724e359870c6644b40e497bdc0589a033220fe15429d885a0")
                .unwrap()
        } else if block_core.height == 91880 && coinbase {
            TxHash::from_hex("e3bf3d07d4b0375638d5f1db5255fe07ba2c4cb067cd81b84ee974b6585fb469")
                .unwrap()
        } else {
            tx.txid()
        };
        txs.push(Tx::from_core_block(block_core, tx_id, &tx));
        for (idx, tx_out) in tx.output.iter().enumerate() {
            outputs.push(Output::from_core_block(
                block_core, tx_id, &tx, idx as u32, tx_out,
            ))
        }
        if !tx.is_coin_base() {
            for (_idx, tx_in) in tx.input.iter().enumerate() {
                inputs.push(Input::from_core_block(block_core, tx_id, tx_in));
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
