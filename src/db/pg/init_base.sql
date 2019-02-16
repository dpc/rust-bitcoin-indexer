CREATE TABLE IF NOT EXISTS indexer_state (
  bulk_mode BOOLEAN NOT NULL
);

CREATE TABLE IF NOT EXISTS block (
  id BIGSERIAL NOT NULL UNIQUE PRIMARY KEY,
  height BIGINT NOT NULL,
  hash BYTEA NOT NULL,
  prev_hash BYTEA NOT NULL,
  merkle_root BYTEA NOT NULL,
  time BIGINT NOT NULL,
  extinct BOOLEAN NOT NULL DEFAULT FALSE
);

-- We always want these two, as a lot of logic is based
-- on `block` table, and it's the smallest table overall,
-- so it doesn't matter that much
CREATE INDEX IF NOT EXISTS block_hash ON block (hash);
CREATE INDEX IF NOT EXISTS block_height ON block (height);
CREATE UNIQUE INDEX IF NOT EXISTS block_hash_not_extinct ON block (hash) WHERE extinct = false;

CREATE TABLE IF NOT EXISTS tx (
  id BIGSERIAL NOT NULL, -- defined as PKEY later
  block_id BIGINT NOT NULL,
  hash BYTEA NOT NULL,
  weight BIGINT NOT NULL,
  fee BIGINT NOT NULL,
  coinbase BOOLEAN NOT NULL
);

CREATE TABLE IF NOT EXISTS output (
  id BIGSERIAL NOT NULL, -- defined as PKEY later
  tx_id BIGINT NOT NULL,
  tx_idx INT NOT NULL,
  value BIGINT NOT NULL,
  address TEXT,
  coinbase BOOLEAN NOT NULL
);

-- Unfortunately `input` is not simply unique on `output_id`, because there can be
-- multiple inputs created for the same output, some of which were extinct
CREATE TABLE IF NOT EXISTS input (
  output_id BIGINT NOT NULL, -- output id this tx input spends
  tx_id BIGINT NOT NULL -- tx id this input is from
);

-- create some views
CREATE OR REPLACE VIEW address_balance AS
  SELECT address, SUM(
    CASE WHEN input.output_id IS NULL THEN value ELSE 0 END
  ) AS value
  FROM output
  JOIN tx AS output_tx ON output.tx_id = output_tx.id
  JOIN block AS output_block ON output_tx.block_id = output_block.id
  LEFT JOIN input
    JOIN tx AS input_tx ON input.tx_id = input_tx.id
    JOIN block AS input_block ON input_tx.block_id = input_block.id
  ON output.id = input.output_id AND input_block.extinct = false
  WHERE
    output_block.extinct = false
  GROUP BY
    output.address;

CREATE OR REPLACE VIEW address_balance_at_height AS
  SELECT address, block.height, SUM(
    CASE WHEN output_block.height <= block.height AND input.output_id IS NULL THEN output.value ELSE 0 END
  ) AS value
  FROM block
  JOIN output ON true
  JOIN tx AS output_tx ON output.tx_id = output_tx.id
  JOIN block AS output_block ON output_tx.block_id = output_block.id
  LEFT JOIN input
    JOIN tx AS input_tx ON input.tx_id = input_tx.id
    JOIN block AS input_block ON input_tx.block_id = input_block.id
  ON output.id = input.output_id AND
    input_block.extinct = false AND
    input_block.height <= block.height
  WHERE
    block.extinct = false AND
    output_block.extinct = false
  GROUP BY
    block.height,
    output.address
  ORDER BY output.address;
