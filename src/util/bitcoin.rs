use crate::{types::*, Hash160};
use bitcoin::util::address;

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
            address::Payload::ScriptHash(
                Hash160::from_slice(&script.as_bytes()[2..22]).expect("correct data"),
            )
        } else if script.is_p2pkh() {
            address::Payload::PubkeyHash(
                Hash160::from_slice(&script.as_bytes()[3..23]).expect("correct data"),
            )
        } else if script.is_p2pk() {
            // no address format for p2kp
            return None;
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
