mod store;

use bitcoin_rpc::BitcoinRpc;
use common_failures::prelude::*;
use common_failures::quick_main;

fn get_rpc() -> bitcoin_rpc::BitcoinRpc {
    bitcoin_rpc::BitcoinRpc::new(
        "http://localhost:8332".into(),
        Some("user".into()),
        Some("magicpassword".into()),
    )
}

pub fn one_iteration(
    rpc: &BitcoinRpc,
    store: &mut store::DataStore,
) -> Result<Option<(store::BlockHeight, store::BlockHash)>> {
    let store_chain_head = store.get_chain_head()?;

    let next_height = if let Some((store_height, store_hash)) = store_chain_head {
        let node_hash = rpc.get_blockhash(store_height)?;
        if store_hash != node_hash {
            store.revert_head()?;
            return Ok(None);
        }
        store_height + 1
    } else {
        0
    };

    let block = store::Block::fetch_by_height(&rpc, next_height)?;
    let hash = block.hash.clone();
    store.insert(next_height, block)?;

    Ok(Some((next_height, hash)))
}

fn run() -> Result<()> {
    let mut store = store::MemDataStore::new();
    let rpc = get_rpc();
    for _ in 0..548391 {
        if let Some((h, hash)) = one_iteration(&rpc, &mut store)? {
            if h % 1000 == 0 {
                println!("Block {}H: {}", h, hash);
            }
        }
    }
    Ok(())
}

quick_main!(run);
