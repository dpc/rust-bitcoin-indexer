use bitcoin::util::hash::Sha256dHash;
pub use default::default;

pub mod bitcoin_core {
    pub use bitcoin::{
        blockdata::{
            block::Block,
            transaction::{Transaction, TxOut},
        },
        consensus::Decodable,
        util::{
            hash::{Hash160, Sha256dHash},
            privkey::Privkey,
        },
    };
}

pub type BlockHeight = u64;
pub type BlockHash = Sha256dHash;
pub type BlockHex = String;
pub type BitcoinCoreBlock = bitcoin::blockdata::block::Block;
pub type TxHash = Sha256dHash;
pub type TxHex = String;

pub struct BlockInfo {
    pub height: BlockHeight,
    pub hash: BlockHash,
    pub block: bitcoin_core::Block,
}

#[derive(Clone, Debug)]
pub struct RpcInfo {
    pub url: String,
    pub user: Option<String>,
    pub password: Option<String>,
}

impl RpcInfo {
    pub fn to_rpc_client(&self) -> bitcoincore_rpc::Client {
        bitcoincore_rpc::Client::new(self.url.clone(), self.user.clone(), self.password.clone())
    }
}
