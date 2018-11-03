#![allow(unused)] // need some cleanup

mod prefetcher;
mod store;
pub mod types;

use common_failures::{prelude::*, quick_main};

fn run_with_prefetching() -> Result<()> {
    let rpc_info = types::RpcInfo {
        url: "http://localhost:8332".into(),
        user: Some("user".into()),
        password: Some("magicpassword".into()),
    };
    let prefetcher = prefetcher::Prefetcher::new(rpc_info, 0)?;
    for (h, hash, _block) in prefetcher {
        if h % 1000 == 0 {
            println!("Block {}H: {}", h, hash);
        }
    }

    Ok(())
}
quick_main!(run_with_prefetching);
