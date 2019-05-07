use bitcoin_indexer::{db, node::fetcher, opts, prelude::*, util::reversed};
use itertools::Itertools;
use std::{borrow::Borrow, sync::Arc};

use common_failures::quick_main;

fn run() -> Result<()> {
    env_logger::init();
    let opts: opts::Opts = structopt::StructOpt::from_args();
    let rpc_info =
        bitcoin_indexer::RpcInfo::new(opts.node_rpc_url, opts.node_rpc_user, opts.node_rpc_pass)?;
    let db = db::pg::establish_connection()?;
    db.execute(
        "ALTER TABLE blocks ADD COLUMN IF NOT EXISTS merkle_root BYTEA",
        &[],
    )?;
    db.execute(
        "ALTER TABLE blocks ADD COLUMN IF NOT EXISTS time BIGINT",
        &[],
    )?;

    let rpc = rpc_info.to_rpc_client()?;
    let fetcher = fetcher::Fetcher::new(Arc::new(rpc), None, None)?;

    for batch in &fetcher.chunks(1000) {
        let transaction = db.transaction()?;
        for (i, item) in batch.enumerate() {
            if i == 0 {
                eprintln!("Block {}H: {}", item.height, item.id);
            }
            db.execute(
                "UPDATE blocks SET time = $1, merkle_root = $2 WHERE hash = $3",
                &[
                    &(i64::from(item.data.header.time)),
                    &reversed(
                        {
                            let borrow: &[u8] = item.data.header.merkle_root.borrow();
                            borrow
                        }
                        .to_vec(),
                    ),
                    &reversed(
                        {
                            let borrow: &[u8] = item.id.borrow();
                            borrow
                        }
                        .to_vec(),
                    ),
                ],
            )?;
        }
        transaction.commit()?;
    }

    db.execute("ALTER TABLE blocks ALTER COLUMN time SET NOT NULL", &[])?;
    db.execute(
        "ALTER TABLE blocks ALTER COLUMN merkle_root SET NOT NULL",
        &[],
    )?;
    Ok(())
}

quick_main!(run);
