#![allow(unused)] // need some cleanup

mod db;
mod opts;
mod prefetcher;
mod prelude;

use crate::prelude::*;
use simple_server::Server;

use common_failures::{prelude::*, quick_main};
use std::thread;

struct Indexer {
    starting_node_height: u64,
    rpc: bitcoincore_rpc::Client,
    rpc_info: RpcInfo,
    db: Box<dyn db::DataStore>,
}

impl Indexer {
    fn new(rpc_info: RpcInfo, db: impl db::DataStore + 'static) -> Result<Self> {
        let mut rpc = bitcoincore_rpc::Client::new(
            rpc_info.url.clone(),
            rpc_info.user.clone(),
            rpc_info.password.clone(),
        );
        let starting_node_height = rpc.get_block_count()?;

        Ok(Self {
            rpc,
            rpc_info,
            starting_node_height,
            db: Box::new(db),
        })
    }

    fn process_block(&mut self, binfo: BlockInfo) -> Result<()> {
        let block_height = binfo.height;
        if block_height >= self.starting_node_height || block_height % 1000 == 0 {
            println!("Block {}H: {}", binfo.height, binfo.hash);
        }

        if let Some(db_hash) = self.db.get_hash_by_height(block_height)? {
            if db_hash != binfo.hash {
                println!("Block {}H: {} - reorg", block_height, binfo.hash);
                self.db.reorg_at_height(block_height);
                self.db.insert(binfo)?;
            }
        } else {
            self.db.insert(binfo)?;
            if block_height > self.starting_node_height {
                // After we've reached the node chain-head, we want everything
                // to appear immediately, even if it's slower
                self.db.flush()?;
            }
        }

        Ok(())
    }

    fn run(&mut self) -> Result<()> {
        let last_known_height = self.db.get_max_height()?.unwrap_or(0);
        // TODO: For hack testing... Turn into cmdline option
        //let last_known_height = self.db.get_max_height()?.unwrap_or(548831);

        let start_from_block = last_known_height.saturating_sub(100); // redo 100 last blocks, in case there was a reorg

        let prefetcher = prefetcher::Prefetcher::new(&self.rpc_info, start_from_block)?;
        for item in prefetcher {
            self.process_block(item)?;
        }

        Ok(())
    }
}

fn run() -> Result<()> {
    let server = Server::new(|request, mut response| {
        Ok(response.body("OK".as_bytes().to_vec())?)
    });
    thread::spawn( move || { server.listen("127.0.0.1", "8082") });
    println!("Listening on Port 8082 for Healthchecks");

    let opts: opts::Opts = structopt::StructOpt::from_args();
    let rpc_info = RpcInfo {
        url: opts.node_rpc_url,
        user: opts.node_rpc_user,
        password: opts.node_rpc_pass,
    };
    println!("Passed Args {}, {:?}, {:?}", rpc_info.url, rpc_info.user, rpc_info.password);
    //let mut db = db::mem::MemDataStore::default();
    let mut db = db::pg::Postresql::new()?;
    let mut indexer = Indexer::new(rpc_info, db)?;
    indexer.run()
}

quick_main!(run);
