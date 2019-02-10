# Bitcoin Indexer

An experiment in indexing Bitcoin, in Rust.

Query blocks using JsonRPC, dump them into Postgres. After reaching
chain-head, keep indexing in real-time and handle reorgs.

WIP, Goals:

* check how much code is realistically necessary
* check how simple/complex can it be
* check Rust ecosystem support (`rust-bitcoin` mostly)
* check performance and see how much can it be optimized
* check unkown unknows and own knowledge


## Running

Install Rust with https://rustup.rs


### Bitcoind node

Setup Bitcoind full node, with a config similiar to this:

```
# [core]
# Run in the background as a daemon and accept commands.
daemon=0

# [rpc]
# Accept command line and JSON-RPC commands.
server=1
# Username for JSON-RPC connections
rpcuser=user
# Password for JSON-RPC connections
rpcpassword=password

# [wallet]
# Do not load the wallet and disable wallet RPC calls.
disablewallet=1
walletbroadcast=0
```

The only important part here is being able to access JSON-RPC interface.

### Postgresql

Setup Postgresql DB, with a db and user:pass that can access it. Example:

```
sudo su postgres
export PGPASSWORD=bitcoin-indexer
createuser bitcoin-indexer
createdb bitcoin-indexer bitcoin-indexer
```

Setup `.env` file with Postgresql settings (URL with password, user, dbname). Example:

```
DATABASE_URL=postgres://bitcoin-indexer:bitcoin-indexer@localhost/bitcoin-indexer
```

#### Optimize DB performance for massive amount of inserts!

**This one is very important!!!**

Indexing from scratch will dump huge amounts of data into the DB.
If you don't want to wait for the initial indexing to complete for days or weeks,
you should carefully review this section.

On software level `pg.rs` already implements the following optimizations:

* **until reaching the chain-head all tables are `UNLOGGED`**; this gives
  great speed boost, but is **not crash-resistant**; keep your Postgresql
  stable; in case of it crashing during initial sync, you will have to
  start from scratch;
* inserts are made using multi-row value inserts;
* multiple multi-row insert statements are batched into one transaction;
* initial sync starts with no indices and utxo set is cached in memory;
  try not to stop the indexer once started;
* once restarted, missing UTXOs are fetched from the db, but new ones
  are still being cached in memory;
* once restarted, only minimum indices are created (for UTXO fetching)
* all indices are created only after reach the chain-head

Tune your system for best performance too:

* [Consider tunning your PG instance][tune-psql] for such workloads.;
* [Make sure your DB is on a performant file-system][perf-fs]; generally COW filesystems perform poorly
  for databases, without providing any value; [on `btrfs` you can disable COW per directory][chattr];
  eg. `chattr -R +C /var/lib/postgresql/9.6/`; On other FSes: disable barriers, align to SSD; you can
  mount your fs in more risky-mode for initial sync, and revert back to safe settings
  aftewards.

[perf-fs]: https://www.slideshare.net/fuzzycz/postgresql-on-ext4-xfs-btrfs-and-zfs
[tune-psql]: https://stackoverflow.com/questions/12206600/how-to-speed-up-insertion-performance-in-postgresql
[chattr]: https://www.kossboss.com/btrfs-disabling-cow-on-a-file-or-directory-nodatacow/

Possibly ask an experienced db admin if anything more can be done. We're talking
about inserting around billion records into 3 tables each.

### Run

Now everything should be ready. Compile and run with:

```
cargo build --release; \
	time ./target/release/bitcoin-indexer \
	--rpc-url http://localhost:8332 \
	--rpc-user user --rpc-pass password
```

in a directory containing the `.env` file.

#### More options

You can use `--wipe-to-height` to wipe history at a point, or `--wipe-whole-db` to wipe the db.

For logging set env. var. `RUST_LOG` to `rust_bitcoin_indexer` or refer to https://docs.rs/env_logger/0.6.0/env_logger/.
**Warning**: Do not turn on logging in production! It's very slow: https://github.com/sebasmagri/env_logger/issues/123 .


### Some useful stuff that can be done already

Check current balance of an address:

```
bitcoin-indexer=> select * from address_balances where address = '14zV5ZCqYmgyCzoVEhRVsP7SpUDVsCBz5g';                                                                                                                                          
              address               |   value
------------------------------------+------------
 14zV5ZCqYmgyCzoVEhRVsP7SpUDVsCBz5g | 6138945213
```

Check balances at a given height:

```
bitcoin-indexer=> select * from address_balances_at_height WHERE address IN ('14zV5ZCqYmgyCzoVEhRVsP7SpUDVsCBz5g', '344tcgkKA97LpgzGtAprtqnNRDfo4VQQWT') AND height = 559834;
              address               | height |   value   
------------------------------------+--------+-----------
 14zV5ZCqYmgyCzoVEhRVsP7SpUDVsCBz5g | 559834 | 162209091
 344tcgkKA97LpgzGtAprtqnNRDfo4VQQWT | 559834 |         0
```
