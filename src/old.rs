// this is all some previous stuff that I was testing
//
// I should remove it later

pub fn sync_backward(rpc: &BitcoinRpc, store: &mut store::DataStore) -> Result<()> {
    let mut height = rpc.getblockcount()?;
    let mut hash = rpc.get_blockhash(height)?;

    loop {
        if let Some(block) = store.get_by_height(height)? {
            if block.hash != hash {
                store.revert_by_height(height)?;
                height -= 1;
                hash = rpc.get_blockhash(height)?;
                continue;
            }
            let min_height = store.get_min_height()?;
            match min_height {
                None => {
                    height -= 1;
                    hash = block.prev_hash;
                    // keep syncing
                },
                Some(0) =>{ return Ok(()); } // just restart the loop,
                Some(x) => {
                    // continue at x
                    height = x;
                    hash = rpc.get_blockhash(height)?;
                }

            }
        }

        let block_hex = rpc.get_block(&hash)?;
        let block_bytes = hex::decode(block_hex)?;
        let block: bitcoin::blockdata::block::Block =
            bitcoin::network::serialize::deserialize(&block_bytes)?;

        let store_block = store::Block::from_core_block(hash.clone(), height, &block);
        store.insert(height, store_block)?;

        if height % 10 == 0 {
            println!("Block {}H: {}", height, hash);
        }
        if height == 0 {
            break;
        }
        height -= 1;
        hash = block.header.prev_blockhash;
    }

    Ok(())
}

pub fn sync_backward_main() -> Result<()> {
    let mut store = store::MemDataStore::new();
    let rpc = get_rpc();

    sync_backward(&rpc, &mut store)
}

pub fn one_iteration_forward(
    rpc: &BitcoinRpc,
    store: &mut store::DataStore,
) -> Result<Option<(BlockHeight, BlockHash)>> {
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
        if let Some((h, hash)) = one_iteration_forward(&rpc, &mut store)? {
            if h % 1000 == 0 {
                println!("Block {}H: {}", h, hash);
            }
        }
    }
    Ok(())
}
