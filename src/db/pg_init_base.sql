CREATE TABLE IF NOT EXISTS indexer_state (
  bulk_mode BOOLEAN NOT NULL,
  height BIGINT
);

CREATE TABLE IF NOT EXISTS blocks (
  id BIGSERIAL NOT NULL UNIQUE PRIMARY KEY,
  height BIGINT NOT NULL,
  hash BYTEA NOT NULL,
  prev_hash BYTEA NOT NULL,
  merkle_root BYTEA NOT NULL,
  time BIGINT NOT NULL,
  orphaned BOOLEAN NOT NULL DEFAULT FALSE
);

-- We always want these two, as a lot of logic is based
-- on `blocks` table, and it's the smallest table overall,
-- so it doesn't matter that much
CREATE UNIQUE INDEX IF NOT EXISTS blocks_hash ON blocks (hash);
CREATE UNIQUE INDEX IF NOT EXISTS blocks_height ON blocks (height);

CREATE TABLE IF NOT EXISTS txs (
  id BIGSERIAL NOT NULL UNIQUE PRIMARY KEY,
  block_id BIGINT NOT NULL,
  hash BYTEA NOT NULL,
  coinbase BOOLEAN NOT NULL
);

CREATE TABLE IF NOT EXISTS outputs (
  id BIGSERIAL NOT NULL UNIQUE PRIMARY KEY,
  tx_id BIGINT NOT NULL,
  tx_idx INT NOT NULL,
  value BIGINT NOT NULL,
  address TEXT,
  coinbase BOOLEAN NOT NULL
);

CREATE TABLE IF NOT EXISTS inputs (
  output_id BIGINT NOT NULL PRIMARY KEY, -- output id this tx input spends
  tx_id BIGINT NOT NULL -- tx id this input is from
);


CREATE OR REPLACE VIEW address_balances AS
  SELECT address, SUM(
    CASE WHEN inputs.output_id IS NULL THEN value ELSE 0 END
  ) AS value
  FROM outputs
  JOIN txs AS output_txs ON outputs.tx_id = output_txs.id
  JOIN blocks AS output_blocks ON output_txs.block_id = output_blocks.id
  LEFT JOIN inputs
    JOIN txs AS input_txs ON inputs.tx_id = input_txs.id
    JOIN blocks AS input_blocks ON input_txs.block_id = input_blocks.id
  ON outputs.id = inputs.output_id AND input_blocks.orphaned = false
  WHERE
    output_blocks.orphaned = false
  GROUP BY
    outputs.address;

CREATE OR REPLACE VIEW address_balances_at_height AS
  SELECT address, blocks.height, SUM(
    CASE WHEN output_blocks.height <= blocks.height AND inputs.output_id IS NULL THEN outputs.value ELSE 0 END
  ) AS value
  FROM blocks
  JOIN outputs ON true
  JOIN txs AS output_txs ON outputs.tx_id = output_txs.id
  JOIN blocks AS output_blocks ON output_txs.block_id = output_blocks.id
  LEFT JOIN inputs
    JOIN txs AS input_txs ON inputs.tx_id = input_txs.id
    JOIN blocks AS input_blocks ON input_txs.block_id = input_blocks.id
  ON outputs.id = inputs.output_id AND
    input_blocks.orphaned = false AND
    input_blocks.height <= blocks.height
  WHERE
    blocks.orphaned = false AND
    output_blocks.orphaned = false
  GROUP BY
    blocks.height,
    outputs.address
  ORDER BY outputs.address;
