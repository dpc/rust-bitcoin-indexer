CREATE TABLE IF NOT EXISTS blocks (
  id BIGSERIAL NOT NULL UNIQUE PRIMARY KEY,
  height BIGINT NOT NULL,
  hash BYTEA NOT NULL,
  prev_hash BYTEA NOT NULL,
  merkle_root BYTEA NOT NULL,
  time BIGINT NOT NULL
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

DROP VIEW IF EXISTS address_balances;

CREATE VIEW address_balances AS
  SELECT address, SUM(
    CASE WHEN inputs.output_id IS NULL THEN value ELSE 0 END
  ) AS value
  FROM outputs
    LEFT JOIN inputs ON outputs.id = inputs.output_id
  GROUP BY
    outputs.address;

DROP VIEW IF EXISTS address_balances_at_height;

CREATE VIEW address_balances_at_height AS
  SELECT address, blocks.height, SUM(
    CASE WHEN outputs.height <= blocks.height AND inputs.output_id IS NULL THEN outputs.value ELSE 0 END
  ) AS value
  FROM blocks
  LEFT JOIN outputs
    ON true
  LEFT JOIN inputs
    ON outputs.id = inputs.output_id
    AND inputs.height <= blocks.height
  GROUP BY
    blocks.height,
    outputs.address
  ORDER BY outputs.address;
