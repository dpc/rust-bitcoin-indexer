#![allow(unused)] // need some cleanup

use bitcoin_indexer::{
    db::{self, DataStore},
    opts, prefetcher,
    prelude::*,
};
use log::info;
use std::sync::Arc;

use common_failures::{prelude::*, quick_main};

struct Indexer {
    node_starting_chainhead_height: u64,
    rpc: Arc<bitcoincore_rpc::Client>,
    db: Box<dyn db::DataStore>,
}

impl Indexer {
    fn new(rpc_info: RpcInfo, db: impl db::DataStore + 'static) -> Result<Self> {
        let rpc = bitcoincore_rpc::Client::new(
            rpc_info.url.clone(),
            rpc_info.user.clone(),
            rpc_info.password.clone(),
        );
        let rpc = Arc::new(rpc);
        let node_starting_chainhead_height = rpc.get_block_count()?;
        info!("Node chain-head at {}H", node_starting_chainhead_height);

        Ok(Self {
            rpc,
            node_starting_chainhead_height,
            db: Box::new(db),
        })
    }

    fn process_block(&mut self, binfo: BlockInfo) -> Result<()> {
        let block_height = binfo.height;
        if block_height >= self.node_starting_chainhead_height || block_height % 1000 == 0 {
            eprintln!("Block {}H: {}", binfo.height, binfo.hash);
        }

        if let Some(db_hash) = self.db.get_hash_by_height(block_height)? {
            if db_hash != binfo.hash {
                info!(
                    "Node block != db block at {}H; {} != {} - reorg",
                    block_height, binfo.hash, db_hash
                );
                self.db.insert(binfo)?;
            }
        } else {
            self.db.insert(binfo)?;
            if block_height >= self.node_starting_chainhead_height {
                if block_height == self.node_starting_chainhead_height {
                    self.db.mode_normal()?;
                }
            }
        }

        Ok(())
    }

    fn run(&mut self) -> Result<()> {
        let start = if let Some(last_indexed_height) = self.db.get_max_height()? {
            info!("Last indexed block {}H", last_indexed_height);

            assert!(last_indexed_height <= self.node_starting_chainhead_height);
            let blocks_to_catch_up = self.node_starting_chainhead_height - last_indexed_height;
            if blocks_to_catch_up <= self.node_starting_chainhead_height / 10 {
                self.db.mode_normal()?;
            } else {
                self.db.mode_bulk()?;
            }
            let start_from_block = last_indexed_height.saturating_sub(100); // redo 100 last blocks, in case there was a reorg
            Some(BlockHeightAndHash {
                height: start_from_block,
                hash: self
                    .db
                    .get_hash_by_height(start_from_block)?
                    .expect("Block hash should be there"),
            })
        } else {
            // test indices dropping and creation
            self.db.mode_fresh()?;
            self.db.mode_bulk()?;
            self.db.mode_normal()?;
            self.db.mode_bulk()?;
            self.db.mode_fresh()?;

            None
        };

        let prefetcher = prefetcher::Prefetcher::new(self.rpc.clone(), start)?;
        for item in prefetcher {
            self.process_block(item)?;
        }

        Ok(())
    }
}

fn run() -> Result<()> {
    env_logger::init();
    let opts: opts::Opts = structopt::StructOpt::from_args();
    let rpc_info = RpcInfo {
        url: opts.node_rpc_url,
        user: opts.node_rpc_user,
        password: opts.node_rpc_pass,
    };
    //let mut db = db::mem::MemDataStore::default();

    if opts.wipe_db {
        db::pg::Postresql::wipe()?;
    } else if let Some(height) = opts.wipe_to_height {
        let mut db = db::pg::Postresql::new()?;
        db.wipe_to_height(height)?;
    } else {
        let mut db = db::pg::Postresql::new()?;
        let mut indexer = Indexer::new(rpc_info, db)?;
        indexer.run()?;
    }

    Ok(())
}

quick_main!(run);
