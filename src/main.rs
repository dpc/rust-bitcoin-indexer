#![allow(unused)] // need some cleanup

mod db;
mod opts;
mod prefetcher;
mod prelude;

use crate::prelude::*;

use common_failures::{prelude::*, quick_main};

struct Indexer {
    starting_node_height: u64,
    rpc: bitcoin_rpc::BitcoinRpc,
    rpc_info: RpcInfo,
    db: Box<dyn db::DataStore>,
}

impl Indexer {
    fn new(rpc_info: RpcInfo, db: impl db::DataStore + 'static) -> Result<Self> {
        let rpc = bitcoin_rpc::BitcoinRpc::new(
            rpc_info.url.clone(),
            rpc_info.user.clone(),
            rpc_info.password.clone(),
        );
        let starting_node_height = rpc.getblockcount()?;

        Ok(Self {
            rpc,
            rpc_info,
            starting_node_height,
            db: Box::new(db),
        })
    }

    fn process_block(&mut self, binfo: &BlockInfo) -> Result<()> {
        if binfo.height < self.starting_node_height || binfo.height % 1000 == 0 {
            println!("Block {}H: {}", binfo.height, binfo.hash);
        }

        if let Some(db_hash) = self.db.get_hash_by_height(binfo.height)? {
            if db_hash != binfo.hash {
                println!("Block {}H: {} - reorg", binfo.height, binfo.hash);
                self.db.reorg_at_height(binfo.height);
            }
        }
        self.db.insert(binfo)?;

        Ok(())
    }

    fn run(&mut self) -> Result<()> {
        let last_known_height = self.db.get_max_height()?.unwrap_or(0);
        // TODO: For hack testing... Turn into cmdline option
        //let last_known_height = self.db.get_max_height()?.unwrap_or(548831);

        let start_from_block = last_known_height.saturating_sub(100); // redo 100 last blocks, in case there was a reorg

        let prefetcher = prefetcher::Prefetcher::new(&self.rpc_info, start_from_block)?;
        for item in prefetcher {
            self.process_block(&item)?;
        }

        Ok(())
    }
}

fn run() -> Result<()> {
    let opts: opts::Opts = structopt::StructOpt::from_args();
    let rpc_info = RpcInfo {
        url: opts.node_rpc_url,
        user: opts.node_rpc_user,
        password: opts.node_rpc_pass,
    };
    let mut db = db::mem::MemDataStore::default();
    let mut indexer = Indexer::new(rpc_info, db)?;
    indexer.run()
}

quick_main!(run);
