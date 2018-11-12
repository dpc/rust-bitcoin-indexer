CREATE TABLE blocks (
  id BIGSERIAL UNIQUE,
  height BIGINT NOT NULL,
  hash TEXT NOT NULL PRIMARY KEY,
  prev_hash TEXT NOT NULL
);

CREATE UNIQUE INDEX ON blocks (height);


CREATE TABLE txs (
  id BIGSERIAL UNIQUE,
  height BIGINT NOT NULL,
  hash TEXT NOT NULL PRIMARY KEY,
  coinbase BOOLEAN NOT NULL
);

CREATE INDEX ON txs (height);


CREATE TABLE outputs (
  id BIGSERIAL UNIQUE,
  height BIGINT NOT NULL,
  tx_hash TEXT NOT NULL,
  tx_idx INT NOT NULL,
  value BIGINT NOT NULL,
  address TEXT,
  coinbase BOOLEAN NOT NULL,
  PRIMARY KEY (tx_hash, tx_idx)
);


CREATE INDEX ON outputs (height);
CREATE INDEX ON outputs (address, value);


CREATE TABLE inputs (
  id BIGSERIAL UNIQUE,
  height BIGINT NOT NULL,
  utxo_tx_hash TEXT NOT NULL,
  utxo_tx_idx INT NOT NULL,
  PRIMARY KEY (utxo_tx_hash, utxo_tx_idx)
);

CREATE INDEX ON inputs (height);
