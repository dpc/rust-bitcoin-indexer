use bitcoin::util::hash::Sha256dHash;

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

pub fn default<T: Default>() -> T {
    std::default::Default::default() // gosh, that's a lot of default, isn't it?
}

pub type BlockHeight = u64;
pub type BlockHash = Sha256dHash;
pub type BlockHex = String;
pub type BitcoinCoreBlock = bitcoin::blockdata::block::Block;

pub struct RpcInfo {
    pub url: String,
    pub user: Option<String>,
    pub password: Option<String>,
}

impl RpcInfo {
    pub fn to_rpc_client(&self) -> bitcoin_rpc::BitcoinRpc {
        bitcoin_rpc::BitcoinRpc::new(self.url.clone(), self.user.clone(), self.password.clone())
    }
}
