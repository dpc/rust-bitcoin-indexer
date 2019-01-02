CREATE TABLE blocks (
  id BIGSERIAL UNIQUE,
  height BIGINT NOT NULL,
  hash BYTEA NOT NULL PRIMARY KEY,
  prev_hash BYTEA NOT NULL
);

CREATE UNIQUE INDEX ON blocks (height);


CREATE TABLE txs (
  id BIGSERIAL UNIQUE,
  height BIGINT NOT NULL,
  hash BYTEA NOT NULL PRIMARY KEY,
  coinbase BOOLEAN NOT NULL
);

CREATE INDEX ON txs (height);


CREATE TABLE outputs (
  id BIGSERIAL UNIQUE,
  height BIGINT NOT NULL,
  tx_id BIGINT NOT NULL,
  tx_idx INT NOT NULL,
  value BIGINT NOT NULL,
  address TEXT,
  coinbase BOOLEAN NOT NULL,
  PRIMARY KEY (tx_id, tx_idx)
);


CREATE INDEX ON outputs (height);
-- TODO: create after initial sync completed
-- CREATE INDEX ON outputs (address, value);


CREATE TABLE inputs (
  output_id BIGINT NOT NULL PRIMARY KEY,
  height BIGINT NOT NULL
);

CREATE INDEX ON inputs (height);
