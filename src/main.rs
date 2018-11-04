#![allow(unused)] // need some cleanup

mod opts;
mod prefetcher;
mod prelude;
mod store;

use crate::prelude::*;

use common_failures::{prelude::*, quick_main};

fn run_with_prefetching(rpc_info: &RpcInfo) -> Result<()> {
    let prefetcher = prefetcher::Prefetcher::new(&rpc_info, 0)?;
    for (h, hash, _block) in prefetcher {
        if h % 1000 == 0 {
            println!("Block {}H: {}", h, hash);
        }
    }

    Ok(())
}

fn run() -> Result<()> {
    let opts: opts::Opts = structopt::StructOpt::from_args();
    let rpc_info = RpcInfo {
        url: opts.node_rpc_url,
        user: opts.node_rpc_user,
        password: opts.node_rpc_pass,
    };
    run_with_prefetching(&rpc_info)
}

quick_main!(run);
