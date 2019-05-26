use bitcoin;
use bitcoin_indexer::{
    db::{self, MempoolStore},
    prelude::*,
    types::WithId,
};
use bitcoincore_rpc::RpcApi;
use std::{collections::HashSet, env};

use common_failures::quick_main;

fn run() -> Result<()> {
    env_logger::init();
    dotenv::dotenv()?;
    let db_url = env::var("DATABASE_URL")?;
    let node_url = env::var("NODE_RPC_URL")?;
    let mut db = db::pg::MempoolStore::new(db_url)?;

    let rpc_info = bitcoin_indexer::RpcInfo::from_url(&node_url)?;

    let rpc = rpc_info.to_rpc_client()?;

    let mut done = HashSet::new();

    loop {
        // TODO: FIXME: Just use LRU instead
        let mut inserted = 0;
        let mut failed = 0;

        if done.len() > 50_000 {
            done.clear();
        }
        for tx_id in rpc.get_raw_mempool()? {
            if done.contains(&tx_id) {
                continue;
            }

            let tx: Option<bitcoin::Transaction> = rpc.get_by_id(&tx_id).ok();
            match db.insert(&WithId {
                id: tx_id,
                data: tx,
            }) {
                Err(e) => {
                    eprintln!("{}", e);
                    failed += 1;
                }
                Ok(()) => {
                    done.insert(tx_id);
                    inserted += 1;
                }
            }
        }
        eprintln!("Scanned mempool; success: {}; failed: {}", inserted, failed);
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}

quick_main!(run);
