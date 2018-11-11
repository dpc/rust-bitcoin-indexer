CREATE TABLE blocks (
  id BIGSERIAL UNIQUE PRIMARY KEY,
  height BIGINT NOT NULL,
  hash TEXT NOT NULL,
  prev_hash TEXT NOT NULL
);

CREATE UNIQUE INDEX ON blocks (hash);
CREATE UNIQUE INDEX ON blocks (height);


CREATE TABLE txs (
  id BIGSERIAL UNIQUE PRIMARY KEY,
  height BIGINT NOT NULL,
  hash TEXT NOT NULL
);

CREATE INDEX ON txs (height);
CREATE UNIQUE INDEX ON txs (hash);


CREATE TABLE outputs (
  id BIGSERIAL UNIQUE PRIMARY KEY,
  height BIGINT NOT NULL,
  tx_hash TEXT NOT NULL,
  tx_idx SMALLINT NOT NULL,
  value BIGINT NOT NULL,
  address TEXT
);


CREATE INDEX ON outputs (height);
CREATE UNIQUE INDEX ON outputs (tx_hash, tx_idx);
CREATE INDEX ON outputs (address, value);


CREATE TABLE inputs (
  id BIGSERIAL UNIQUE PRIMARY KEY,
  height BIGINT NOT NULL,
  utxo_tx_hash BIGINT NOT NULL,
  utxo_tx_idx SMALLINT NOT NULL
);

CREATE INDEX ON inputs (height);
CREATE UNIQUE INDEX ON inputs (utxo_tx_hash, utxo_tx_idx);
