CREATE TABLE blocks (
  id BIGSERIAL,
  height BIGINT NOT NULL,
  hash BYTEA NOT NULL,
  prev_hash BYTEA NOT NULL,
  time TIMESTAMP NOT NULL,
  PRIMARY KEY (time, hash)
);

--SELECT CREATE_HYPERTABLE('blocks', 'time', chunk_time_interval => interval '1 day');
--CREATE UNIQUE INDEX ON blocks (time DESC, id);
--CREATE UNIQUE INDEX ON blocks (time DESC, height);


CREATE TABLE txs (
  id BIGSERIAL,
  height BIGINT NOT NULL,
  hash BYTEA NOT NULL,
  coinbase BOOLEAN NOT NULL,
  time TIMESTAMP NOT NULL,
  PRIMARY KEY (time, hash)
);

--SELECT CREATE_HYPERTABLE('txs', 'time', chunk_time_interval => interval '1 day');
--CREATE UNIQUE INDEX ON txs (time DESC, id);
--CREATE INDEX ON txs (time DESC, height);


CREATE TABLE outputs (
  id BIGSERIAL UNIQUE,
  height BIGINT NOT NULL,
  tx_hash BYTEA NOT NULL,
  tx_idx INT NOT NULL,
  value BIGINT NOT NULL,
  address TEXT,
  coinbase BOOLEAN NOT NULL,
  PRIMARY KEY (tx_hash, tx_idx)
);

--CREATE INDEX ON outputs (height);
--CREATE INDEX ON outputs (address, value);


CREATE TABLE inputs (
  id BIGSERIAL UNIQUE,
  height BIGINT NOT NULL,
  utxo_tx_hash BYTEA NOT NULL,
  utxo_tx_idx INT NOT NULL,
  PRIMARY KEY (utxo_tx_hash, utxo_tx_idx)
);

--CREATE INDEX ON inputs (height);
