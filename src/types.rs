pub use bitcoin_hashes::{hex::FromHex as _, Hash as _};

/// Data in a block
///
/// Comes associated with height and hash of the block.
///
/// `T` is type type of the data.
pub struct WithHeightAndId<H, D = ()> {
    pub height: BlockHeight,
    pub id: H,
    pub data: D,
}

pub struct WithId<H, D = ()> {
    pub id: H,
    pub data: D,
}


pub type WithHash<T> = WithId<Sha256dHash, T>;

pub type BlockHeight = u32;
pub type BlockHash = Sha256dHash;

pub struct BlockHeightAndHash {
    pub height: BlockHeight,
    pub hash: BlockHash,
}

/// Block data from BitcoinCore (`rust-bitcoin`)
pub type BlockData = WithHeightAndId<BlockHash, Box<bitcoin::Block>>;

pub use bitcoin_hashes::{hash160::Hash as Hash160, sha256d::Hash as Sha256dHash};
pub type BlockHex = String;
pub type TxHex = String;
pub type TxHash = Sha256dHash;
