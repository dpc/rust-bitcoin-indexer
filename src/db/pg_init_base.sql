CREATE TABLE IF NOT EXISTS blocks (
  id BIGSERIAL NOT NULL UNIQUE PRIMARY KEY,
  height BIGINT NOT NULL,
  hash BYTEA NOT NULL,
  prev_hash BYTEA NOT NULL
);

-- We always want these two, as a lot of logic is based
-- on `blocks` table, and it's the smallest table overall,
-- so it doesn't matter that much
CREATE UNIQUE INDEX IF NOT EXISTS blocks_hash ON blocks (hash);
CREATE UNIQUE INDEX IF NOT EXISTS blocks_height ON blocks (height);

CREATE TABLE IF NOT EXISTS txs (
  id BIGSERIAL NOT NULL UNIQUE PRIMARY KEY,
  height BIGINT NOT NULL,
  hash BYTEA NOT NULL,
  coinbase BOOLEAN NOT NULL
);

CREATE TABLE IF NOT EXISTS outputs (
  id BIGSERIAL NOT NULL UNIQUE PRIMARY KEY,
  height BIGINT NOT NULL,
  tx_id BIGINT NOT NULL,
  tx_idx INT NOT NULL,
  value BIGINT NOT NULL,
  address TEXT,
  coinbase BOOLEAN NOT NULL
);

CREATE TABLE IF NOT EXISTS inputs (
  output_id BIGINT NOT NULL PRIMARY KEY,
  height BIGINT NOT NULL
);
