
pub use bitcoin_hashes::hex::FromHex as _;
pub use bitcoin_hashes::Hash as _;

pub use bitcoin::{
    blockdata::{
        transaction::{Transaction, TxIn, TxOut},
    },
    consensus::Decodable,
    util::key::PrivateKey,
};

/// Data in a block
///
/// Comes associated with height and hash of the block.
///
/// `T` is type type of the data.
pub struct Block<H, D = ()> {
    pub height: BlockHeight,
    pub id: H,
    pub data: D,
}

pub type BlockHeight = u64;
pub type BlockHash = Sha256dHash;
pub struct BlockHeightAndHash {
    pub height: BlockHeight,
    pub hash: BlockHash,
}

/// Block data from BitcoinCore (`rust-bitcoin`)
pub type BlockCore = Block<BlockHash, bitcoin::blockdata::block::Block>;

pub use bitcoin_hashes::hash160::Hash as Hash160;
pub use bitcoin_hashes::sha256d::Hash as Sha256dHash;
pub type BlockHex = String;
pub type BitcoinCoreBlock = bitcoin::blockdata::block::Block;
pub type TxHash = Sha256dHash;
pub type TxHex = String;
pub type OutPoint = bitcoin::blockdata::transaction::OutPoint;
