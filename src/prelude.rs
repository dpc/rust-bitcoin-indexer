use bitcoin::util::address;
pub use default::default;
pub use insideout::InsideOut;
use failure::bail;

pub mod bitcoin_core {
    pub use bitcoin::{
        blockdata::{
            block::Block,
            transaction::{Transaction, TxIn, TxOut},
        },
        consensus::Decodable,
        util::key::PrivateKey,
    };
    pub use bitcoin_hashes::hash160::Hash as Hash160;
}

pub type BlockHeight = u64;
pub type BlockHash = Sha256dHash;
pub struct BlockHeightAndHash {
    pub height: BlockHeight,
    pub hash: BlockHash,
}

pub use bitcoin_hashes::hash160::Hash as Hash160;
pub use bitcoin_hashes::hex::FromHex as _;
pub use bitcoin_hashes::Hash as _;
pub use bitcoin_hashes::sha256d::Hash as Sha256dHash;
pub type BlockHex = String;
pub type BitcoinCoreBlock = bitcoin::blockdata::block::Block;
pub type TxHash = Sha256dHash;
pub type TxHex = String;
pub type OutPoint = bitcoin::blockdata::transaction::OutPoint;

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

pub use common_failures::Result;

/// Block data from BitcoinCore (`rust-bitcoin`)
pub type BlockCore = Block<BlockHash, bitcoin_core::Block>;

#[derive(Clone, Debug)]
pub struct RpcInfo {
    pub url: String,
    pub auth: bitcoincore_rpc::Auth,
}

impl RpcInfo {
    pub fn new(url: String, user: Option<String>, pass: Option<String>) -> Result<Self> {
        let auth = match (user, pass) {
            (Some(u), Some(p)) => bitcoincore_rpc::Auth::UserPass(u.clone(), p.clone()),
            (None, None) => bitcoincore_rpc::Auth::None,
            _ => bail!("Incorrect node auth parameters"),
        };
        Ok(Self {url, auth })
    }
    pub fn to_rpc_client(&self) -> Result<bitcoincore_rpc::Client> {
        Ok(bitcoincore_rpc::Client::new(self.url.clone(), self.auth.clone())?)
    }
}

fn bech_network(
    network: bitcoin::network::constants::Network,
) -> bitcoin_bech32::constants::Network {
    use bitcoin::network::constants::Network;
    match network {
        Network::Bitcoin => bitcoin_bech32::constants::Network::Bitcoin,
        Network::Testnet => bitcoin_bech32::constants::Network::Testnet,
        Network::Regtest => bitcoin_bech32::constants::Network::Regtest,
    }
}

/// Retrieve an address from the given script.
pub fn address_from_script(
    script: &bitcoin::blockdata::script::Script,
    network: bitcoin::network::constants::Network,
) -> Option<address::Address> {
    Some(address::Address {
        payload: if script.is_p2sh() {
            address::Payload::ScriptHash(Hash160::from_slice(&script.as_bytes()[2..22]).expect("correct data"))
        } else if script.is_p2pkh() {
            address::Payload::PubkeyHash(Hash160::from_slice(&script.as_bytes()[3..23]).expect("correct data"))
        } else if script.is_p2pk() {
            // no address format for p2kp
            return None
        } else if script.is_v0_p2wsh() {
            address::Payload::WitnessProgram(
                bitcoin_bech32::WitnessProgram::new(
                    bitcoin_bech32::u5::try_from_u8(0).expect("0<32"),
                    script.as_bytes()[2..34].to_vec(),
                    bech_network(network),
                )
                .unwrap(),
            )
        } else if script.is_v0_p2wpkh() {
            address::Payload::WitnessProgram(
                bitcoin_bech32::WitnessProgram::new(
                    bitcoin_bech32::u5::try_from_u8(0).expect("0<32"),
                    script.as_bytes()[2..22].to_vec(),
                    bech_network(network),
                )
                .unwrap(),
            )
        } else {
            return None;
        },
        network,
    })
}
