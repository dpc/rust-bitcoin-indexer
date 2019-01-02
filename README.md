# Bitcoin Indexer

An experiment in indexing Bitcoin, in Rust.


Query blocks using JsonRPC, dump them into Postgres. Handle
reorg detections.


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
txindex=1

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

`txindex=1` shouldn't be neccessary, and the only important part here
is being able to access RPC interface.

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

#### Optimize DB performance for massive inserts

Indexing from scratch will dump huge amounts of data into the DB via
multi-value `INSERT` statements batched in transactions.

[Consider tunning your PG instance](https://stackoverflow.com/questions/12206600/how-to-speed-up-insertion-performance-in-postgresql)
for such workloads.

### Run
Now everything should be ready. Compile and run with:

```
cargo build --release; \
	and time ./target/release/rust-bitcoin-indexer \
	--rpc-url http://localhost:8332 \
	--rpc-user user --rpc-pass password
```


You can use `--init-db` and `--wipe-db` to initialize and wipe the db.
