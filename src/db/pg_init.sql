CREATE TABLE blocks (
  id BIGSERIAL NOT NULL UNIQUE PRIMARY KEY,
  height BIGINT NOT NULL,
  hash BYTEA NOT NULL,
  prev_hash BYTEA NOT NULL
);

-- We always want these two, as a lot of logic is based
-- on `blocks` table, and it's the smallest table overall,
-- so it doesn't matter that much
CREATE UNIQUE INDEX ON blocks (hash);
CREATE UNIQUE INDEX ON blocks (height);

CREATE TABLE txs (
  id BIGSERIAL NOT NULL UNIQUE PRIMARY KEY,
  height BIGINT NOT NULL,
  hash BYTEA NOT NULL,
  coinbase BOOLEAN NOT NULL
);



CREATE TABLE outputs (
  id BIGSERIAL NOT NULL UNIQUE PRIMARY KEY,
  height BIGINT NOT NULL,
  tx_id BIGINT NOT NULL,
  tx_idx INT NOT NULL,
  value BIGINT NOT NULL,
  address TEXT,
  coinbase BOOLEAN NOT NULL
);




CREATE TABLE inputs (
  output_id BIGINT NOT NULL PRIMARY KEY,
  height BIGINT NOT NULL
);

