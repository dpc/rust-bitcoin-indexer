# Bitcoin Indexer

An experiment in creating a perfect Bitcoin Indexer, in Rust.

Query blocks using JsonRPC, dump them into Postgres in an append-only format,
suitable for querying, as much as event-sourcing-like handling. After reaching
chain-head, keep indexing in real-time and handle reorgs.

This started as an educational experiment, but is quite advanced already.

Goals:

* simplicity and small code:
    * easy to fork and customize
* versatile data model:
    * append-only log
    * support for event sourcing
    * support for efficient queries
    * correct by construction
    * always-coherent state view (i.e. atomic reorgs)
* top-notch performance, especially during initial indexing

Read [How to interact with a blockchain](https://dpc.pw/rust-bitcoin-indexer-how-to-interact-with-a-blockchain) for knowledge sharing, discoveries and design-decisions.

Status:

* The codebase is very simple, design quite clean and composable and performance really good.
* Some tests are there and everything seems quite robust, but not tested in production so far. 
* Indexing the blockchain works
* Indexing mempool works

## Comparing to alternatives

Please take with a grain of salt, and submit PRs if any information is stale or wrong.

### vs [electrs](https://github.com/romanz/electrs)

Electrs uses an embedded key value store (RocksDB), while rust-bitcoin-indexer uses a normal relational data model in a Postgres that can run on a different host/cluster.

Embedded KV store can be potentially more compact and faster, but electrs stores the actual block data, while rust-bitcoin-indexer extracts the data it needs only and throws everything else away. Missing data could be retro-fitted from the blockchain if needed, by adding more columns and writting small program to reindex and back-fill it.

Using relational database will allow you to execute ad-hoc queries and potentially share the db with other applications, without building a separate interface.

rust-bitcoin-indexer was designed to have a good idempotent and reliable events streaming/subscription data model. 

Electrs is used for practical purposes, rust-bitcoin-indexer (at least right now) is just a neat experiment that went far.

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

### `.env` file

Setup `.env` file with Postgresql and Bitcoin Core connection data. Example:

```
DATABASE_URL=postgres://bitcoin-indexer:bitcoin-indexer@localhost/bitcoin-indexer
NODE_RPC_URL=http://someuser:somepassword@localhost:18443
```

#### Optimize DB performance for massive amount of inserts!

**This one is very important!!!**

Indexing from scratch will dump huge amounts of data into the DB.
If you don't want to wait for the initial indexing to complete for days or weeks,
you should carefully review this section.

On software level `pg.rs` already implements the following optimizations:

* inserts are made using multi-row value insert statements;
* multiple multi-row insert statements are batched into one transaction;
* initial sync starts with no indices and utxo set is cached in memory;
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
* on my HDD ext4 `sudo tune2fs -o journal_data_writeback  /dev/<partition>` some improvement, but ultimately
  SMR disk turned out to be too slow to keep up with inserting data into > 1B records table and updating
  indices.

[perf-fs]: https://www.slideshare.net/fuzzycz/postgresql-on-ext4-xfs-btrfs-and-zfs
[tune-psql]: https://stackoverflow.com/questions/12206600/how-to-speed-up-insertion-performance-in-postgresql
[chattr]: https://www.kossboss.com/btrfs-disabling-cow-on-a-file-or-directory-nodatacow/

Possibly ask an experienced db admin if anything more can be done. We're talking
about inserting around billion records into 3 tables each.

For reference -  on my system, I get around 30k txs indexed per second:

```
[2019-05-24T05:20:29Z INFO  bitcoin_indexer::db::pg] Block 194369H fully indexed and commited; 99block/s; 30231tx/s
```

which leads to around 5 hours initial blockchain indexing time (current block height is around 577k)...
and then just 4 additional hours to build indices.

I suggest using `tx/s` metric to estimate completion.

### Run

Now everything should be ready. Compile and run with:

```
cargo run --release --bin bitcoin-indexer
```

After the initial full sync, you can also start mempool indexer:

```
cargo run --release --bin mempool-indexer
```

in a directory containing the `.env` file.

#### More options

You can use `--wipe-whole-db` to wipe the db. (to be removed in the future)

For logging set env. var. `RUST_LOG` to `bitcoin_indexer=info` or refer to https://docs.rs/env_logger/0.6.0/env_logger/.


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

Check txes pending in the mempool:

```
bitcoin-indexer=> select * from tx_in_mempool order by (fee/weight) desc limit 5;
              hash_id               |             hash_rest              | size | weight |  fee   | locktime | coinbase |                                hash                                |             ts
------------------------------------+------------------------------------+------+--------+--------+----------+----------+--------------------------------------------------------------------+----------------------------
 \x5d094cc3e4d9a1ec2d280aa9204ff8e4 | \xf0e960ddfed324880dac6e8ab54544c6 |  191 |    764 | 562871 |   577816 | f        | \xc64445b58a6eac0d8824d3fedd60e9f0e4f84f20a90a282deca1d9e4c34c095d | 2019-05-26 05:31:08.658908
 \x5a8caef874d1cd657460f754a9ae6985 | \xcce821c1789f20b665fd6240717340b6 |  213 |    855 | 182000 |        0 | f        | \xb64073714062fd65b6209f78c121e8cc8569aea954f7607465cdd174f8ae8c5a | 2019-05-26 05:36:10.331908
 \xab60acf5b6c7d6103e7186a9d319210b | \xdb0baedb6c49cfe748411ea8de8425a8 |  371 |   1484 | 298600 |        0 | f        | \xa82584dea81e4148e7cf496cdbae0bdb0b2119d3a986713e10d6c7b6f5ac60ab | 2019-05-26 05:30:57.902214
 \x5b63601004575830ccde165a924b823e | \xd5085b0193da8a5c2d0d1bc579a77a06 |  339 |   1356 | 163953 |        0 | f        | \x067aa779c51b0d2d5c8ada93015b08d53e824b925a16decc305857041060635b | 2019-05-26 05:34:34.603281
 \xaf8ce41bd3c7db9dd44fa126b5d5d386 | \x1b0a8db2986f1124c2ebba3e9cbc9251 |  112 |    450 |  53788 |        0 | f        | \x5192bc9c3ebaebc224116f98b28d0a1b86d3d5b526a14fd49ddbc7d31be48caf | 2019-05-26 05:37:46.736696
(5 rows)
```

and many more. Refer to `./src/db/pg/*.sql` files for good overview of the schema and utilities.

# Support

If you like and/or use this project, you can pay for it by sending Bitcoin to
[33A9SwFHWEnwmFfgRfHu1GvSfCeDcABx93](bitcoin:33A9SwFHWEnwmFfgRfHu1GvSfCeDcABx93).

