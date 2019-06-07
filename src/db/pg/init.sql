-- fresh/base schema: a schema we populate an empty db with

-- **Important**:
-- * all columns sorted by size to minimize padding (https://stackoverflow.com/questions/2966524/calculating-and-saving-space-in-postgresql/7431468#7431468)
-- signgle record table to keep persistent indexer state
CREATE TABLE IF NOT EXISTS indexer_state (
  bulk_mode BOOLEAN NOT NULL
);

-- events: append only
-- you can follow them one by one,
-- to follow blockchain state
-- canceling protocol is used
-- https://github.com/dpc/rust-bitcoin-indexer/wiki/How-to-interact-with-a-blockchain#canceling-protocol
CREATE TABLE IF NOT EXISTS event (
  indexed_ts TIMESTAMP NOT NULL DEFAULT (timezone('utc', now())),
  id BIGSERIAL NOT NULL UNIQUE PRIMARY KEY,
  revert BOOLEAN NOT NULL DEFAULT FALSE,
  block_hash_id BYTEA NOT NULL
);
CREATE INDEX IF NOT EXISTS event_indexed_ts ON event (indexed_ts);

-- blocks: insert only
CREATE TABLE IF NOT EXISTS block (
  time BIGINT NOT NULL, -- time from the block itself
  height INT NOT NULL,
  extinct BOOLEAN NOT NULL DEFAULT FALSE, -- this is the only mutable column in this table
  hash_id BYTEA NOT NULL UNIQUE PRIMARY KEY, -- the hash is split in two to save when referencing in other columns
  hash_rest BYTEA NOT NULL,
  prev_hash_id BYTEA NOT NULL,
  merkle_root BYTEA NOT NULL
);

-- We always want these two, as a lot of logic is based
-- on `block` table, and it's the smallest table overall,
-- so it doesn't matter that much (perforamnce wise)
CREATE INDEX IF NOT EXISTS block_height ON block (height);
CREATE UNIQUE INDEX IF NOT EXISTS block_height_for_not_extinct ON block (height) WHERE extinct = false;
CREATE INDEX IF NOT EXISTS block_extinct ON block (extinct);


-- block -> tx: insert only
-- mapping between blocks and txes they include
CREATE TABLE IF NOT EXISTS block_tx (
  block_hash_id BYTEA NOT NULL,
  tx_hash_id BYTEA NOT NULL
);

-- txs: insert only
CREATE TABLE IF NOT EXISTS tx (
  mempool_ts TIMESTAMP DEFAULT NULL, -- NULL if it was indexed from an indexed block
  fee BIGINT NOT NULL,
  locktime BIGINT NOT NULL,
  current_height INT, -- Warning: mutable! But useful enough to keep it: especialy useful for mempool queries
  weight INT NOT NULL,
  coinbase BOOLEAN NOT NULL,
  hash_id BYTEA NOT NULL,
  hash_rest BYTEA NOT NULL
);

-- outputs: insert only
CREATE TABLE IF NOT EXISTS output (
  value BIGINT NOT NULL,
  tx_idx INT NOT NULL,
  tx_hash_id BYTEA NOT NULL,
  address TEXT
);

-- input: insert only
CREATE TABLE IF NOT EXISTS input (
  output_tx_idx INT NOT NULL,
  has_witness BOOLEAN NOT NULL,
  output_tx_hash_id BYTEA NOT NULL, -- output id this tx input spends
  tx_hash_id BYTEA NOT NULL -- tx id this input is from
);
