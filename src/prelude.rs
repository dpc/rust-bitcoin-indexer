use bitcoin::util::{address, hash::Sha256dHash};
pub use default::default;
pub use insideout::InsideOut;

pub mod bitcoin_core {
    pub use bitcoin::{
        blockdata::{
            block::Block,
            transaction::{Transaction, TxIn, TxOut},
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
pub type OutPoint = bitcoin::blockdata::transaction::OutPoint;

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

fn bech_network(
    network: bitcoin::network::constants::Network,
) -> bitcoin_bech32::constants::Network {
    use bitcoin::network::constants::Network;
    match network {
        Network::Bitcoin => bitcoin_bech32::constants::Network::Bitcoin,
        Network::Testnet => bitcoin_bech32::constants::Network::Testnet,
        Network::Regtest => bitcoin_bech32::constants::Network::Regtest,
        _ => panic!("unsupported network"),
    }
}

/// Retrieve an address from the given script.
pub fn address_from_script(
    script: &bitcoin::blockdata::script::Script,
    network: bitcoin::network::constants::Network,
) -> Option<address::Address> {
    Some(address::Address {
        payload: if script.is_p2sh() {
            address::Payload::ScriptHash(script.as_bytes()[2..22].into())
        } else if script.is_p2pkh() {
            address::Payload::PubkeyHash(script.as_bytes()[3..23].into())
        } else if script.is_p2pk() {
            let secp = secp256k1::Secp256k1::without_caps();
            let pubkey = match secp256k1::key::PublicKey::from_slice(
                &secp,
                &script.as_bytes()[1..(script.len() - 1)],
            ) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Couldn't parse public-key in script {}; {}", script, e);
                    return None;
                }
            };
            address::Payload::Pubkey(pubkey)
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
