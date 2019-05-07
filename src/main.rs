#![allow(unused)] // need some cleanup

use bitcoin_indexer::{
    db::{self, DataStore},
    node::prefetcher,
    opts,
    prelude::*,
    RpcInfo,
    types::*,
};
use log::info;
use std::sync::Arc;
use bitcoincore_rpc::RpcApi;
use bitcoin_indexer::util::BottleCheck;

use common_failures::{prelude::*, quick_main};

struct Indexer {
    node_starting_chainhead_height: u64,
    rpc: Arc<bitcoincore_rpc::Client>,
    db: Box<dyn db::DataStore>,
    bottlecheck_db: BottleCheck,
}

impl Indexer {
    fn new(rpc_info: RpcInfo) -> Result<Self> {
        let rpc = rpc_info.to_rpc_client()?;
        let rpc = Arc::new(rpc);
        let node_starting_chainhead_height = rpc.get_block_count()?;
        let mut db = db::pg::Postresql::new(node_starting_chainhead_height)?;
        info!("Node chain-head at {}H", node_starting_chainhead_height);

        Ok  (Self {
            rpc,
            node_starting_chainhead_height,
            db: Box::new(db),
            bottlecheck_db: BottleCheck::new("database".into()),
        })
    }

    fn process_block(&mut self, block: BlockCore) -> Result<()> {
        let block_height = block.height;
        if block_height >= self.node_starting_chainhead_height || block_height % 1000 == 0 {
            eprintln!("Block {}H: {}", block.height, block.id);
        }

        let Self {
            ref mut db,
            ref mut bottlecheck_db,
            ..
        } = self;

        bottlecheck_db.check(|| db.insert(block))?;
        Ok(())
    }

    fn run(&mut self) -> Result<()> {
        let start = if let Some(last_indexed_height) = self.db.get_head_height()? {
            info!("Last indexed block {}H", last_indexed_height);

            assert!(last_indexed_height <= self.node_starting_chainhead_height);
            let start_from_block = last_indexed_height.saturating_sub(100); // redo 100 last blocks, in case there was a reorg
            Some(Block {
                height: start_from_block,
                id: self
                    .db
                    .get_hash_by_height(start_from_block)?
                    .expect("Block hash should be there"),
                data: (),
            })
        } else {
            None
        };

        let prefetcher = prefetcher::Prefetcher::new(self.rpc.clone(), start)?;
        let mut bottlecheck_fetcher = BottleCheck::new("block fetcher".into());
        for item in bottlecheck_fetcher.check_iter(prefetcher) {
            self.process_block(item)?;
        }

        Ok(())
    }
}

fn run() -> Result<()> {
    env_logger::init();
    let opts: opts::Opts = structopt::StructOpt::from_args();
    let rpc_info = bitcoin_indexer::RpcInfo::new(
        opts.node_rpc_url,
        opts.node_rpc_user,
        opts.node_rpc_pass,
    )?;

    if opts.wipe_db {
        db::pg::Postresql::wipe()?;
    } else if let Some(height) = opts.wipe_to_height {
        let mut indexer = Indexer::new(rpc_info)?;
        indexer.db.wipe_to_height(height)?;
    } else {
        let mut indexer = Indexer::new(rpc_info)?;
        indexer.run()?;
    }

    Ok(())
}

quick_main!(run);
