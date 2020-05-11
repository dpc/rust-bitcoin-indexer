use crate::prelude::*;
use bitcoin::util::address;

pub fn address_from_script(
    script: &bitcoin::blockdata::script::Script,
    network: bitcoin::network::constants::Network,
) -> Option<address::Address> {
    address::Payload::from_script(script).map(|payload| address::Address { payload, network })
}

pub fn network_from_str(s: &str) -> Result<bitcoin::Network> {
    Ok(match s {
        "main" => bitcoin::Network::Bitcoin,
        "test" => bitcoin::Network::Testnet,
        "regtest" => bitcoin::Network::Regtest,
        _ => bail!("Unknown bitcoin chain {}", s),
    })
}
