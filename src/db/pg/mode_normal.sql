-- normal mode schema: after reaching chainhead/first reorg
-- we build all indices, etc. to enable all the queries etc.
-- we also define some utitlity functions


--
-- Keys & indices
--
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'block_tx' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE block_tx ADD PRIMARY KEY (block_hash_id, tx_hash_id);
  END IF;
END $$;
CREATE UNIQUE INDEX IF NOT EXISTS block_tx_tx_hash_id_block_hash_id ON block_tx (tx_hash_id, block_hash_id);

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'tx' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE tx ADD PRIMARY KEY (hash_id);
  END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS tx_hash_id ON tx (hash_id);
CREATE INDEX IF NOT EXISTS tx_coinbase ON tx (coinbase);
CREATE INDEX IF NOT EXISTS tx_mempool_ts ON tx (mempool_ts);
CREATE INDEX IF NOT EXISTS tx_current_height ON tx (current_height);


DO $$
BEGIN
  IF NOT EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'output' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE output ADD PRIMARY KEY (tx_hash_id, tx_idx);
  END IF;
END $$;
CREATE INDEX IF NOT EXISTS output_address_value ON output (address, value);
CREATE INDEX IF NOT EXISTS output_value_address ON output (value, address);

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'input' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE input ADD PRIMARY KEY (output_tx_hash_id, output_tx_idx);
  END IF;
END $$;
CREATE INDEX IF NOT EXISTS input_tx_hash_id ON input (tx_hash_id);


--
-- Utilities
--

-- https://stackoverflow.com/a/25137344/134409
CREATE OR REPLACE FUNCTION reverse_bytes_iter(bytes bytea, length int, midpoint int, index int)
RETURNS bytea AS
$$
  SELECT CASE WHEN index >= midpoint THEN bytes ELSE
    reverse_bytes_iter(
      set_byte(
        set_byte(bytes, index, get_byte(bytes, length-index)),
        length-index, get_byte(bytes, index)
      ),
      length, midpoint, index + 1
    )
  END;
$$ LANGUAGE SQL IMMUTABLE;
CREATE OR REPLACE FUNCTION reverse_bytes(bytes bytea) RETURNS bytea AS
'SELECT reverse_bytes_iter(bytes, octet_length(bytes)-1, octet_length(bytes)/2, 0)'
LANGUAGE SQL IMMUTABLE;

CREATE OR REPLACE FUNCTION hash_from_parts(hash_id bytea, hash_rest bytea) RETURNS bytea AS
'SELECT reverse_bytes_iter(hash_id || hash_rest, octet_length(hash_id || hash_rest)-1, octet_length(hash_id || hash_rest)/2, 0)'
LANGUAGE SQL IMMUTABLE;
CREATE OR REPLACE FUNCTION hash_to_hash_id(hash bytea) RETURNS bytea AS
'SELECT reverse_bytes(substring(hash, 17, 32))'
LANGUAGE SQL IMMUTABLE;


CREATE OR REPLACE VIEW tx_with_hash AS
  SELECT *,
  reverse_bytes(hash_id || hash_rest) AS hash
  FROM tx;

CREATE OR REPLACE VIEW block_with_hash AS
  SELECT *,
  reverse_bytes(hash_id || hash_rest) AS hash
  FROM block;

-- tx joined all the way to the block
-- NOTE: there might be from 0 (NULL data),
-- to many blocks which happaned to include the tx (extinct blocks)
CREATE OR REPLACE VIEW tx_maybe_with_block AS
  SELECT tx.*,
  reverse_bytes(tx.hash_id || tx.hash_rest) AS hash,
  block.hash_id AS block_hash_id,
  block.hash_rest AS block_hash_rest,
  block.height AS block_height,
  block.prev_hash_id AS block_prev_hash_id,
  block.merkle_root AS block_merkle_root,
  block.extinct AS block_extinct,
  block.time AS block_time, -- unix time from header
  (SELECT min(indexed_ts) FROM event WHERE block_hash_id = block.hash_id) AS block_indexed_ts,
  CASE WHEN tx.mempool_ts IS NULL THEN to_timestamp(block.time) ELSE tx.mempool_ts END AS ts, -- if we seen it first, mepool_ts, otherwise official ts from header
  CASE WHEN tx.mempool_ts IS NULL THEN (SELECT min(indexed_ts) FROM event WHERE block_hash_id = block.hash_id) ELSE tx.mempool_ts END AS indexed_ts -- our indexed time - either of block, or from mempool
  FROM tx
  LEFT JOIN block_tx
    JOIN block ON block.hash_id = block_tx.block_hash_id
  ON block_tx.tx_hash_id = tx.hash_id;

CREATE OR REPLACE VIEW tx_with_block AS
  SELECT * FROM tx_maybe_with_block WHERE block_hash_id IS NOT NULL;

-- txes in the mempool
-- select all txes that have null `current_height`, and which outputs were not used by any other tx yet
CREATE OR REPLACE VIEW tx_hash_ids_in_mempool AS
  SELECT
    tx.hash_id
  FROM tx
  JOIN input ON input.tx_hash_id = tx.hash_id
  LEFT JOIN tx AS other_tx ON (other_tx.hash_id = input.tx_hash_id AND other_tx.hash_id <> tx.hash_id AND other_tx.current_height IS NOT NULL)
  WHERE tx.current_height IS NULL
  GROUP BY tx.hash_id
  HAVING count(other_tx.hash_id) = 0;
CREATE OR REPLACE VIEW tx_in_mempool AS
  SELECT
    *
  FROM tx
  WHERE
    hash_id IN (SELECT * FROM tx_hash_ids_in_mempool);
CREATE OR REPLACE VIEW tx_with_hash_in_mempool AS
  SELECT
    *
  FROM tx_with_hash
  WHERE
    hash_id IN (SELECT * FROM tx_hash_ids_in_mempool);

CREATE OR REPLACE VIEW address_balance AS
  SELECT address, SUM(
    CASE WHEN input.output_tx_hash_id IS NULL THEN value ELSE 0 END
  ) AS value
  FROM output
  JOIN tx AS output_tx ON output_tx.hash_id = output.tx_hash_id
  JOIN block_tx AS output_block_tx ON output_block_tx.tx_hash_id = output_tx.hash_id
  JOIN block AS output_block ON output_block.hash_id = output_block_tx.block_hash_id
  LEFT JOIN input
    JOIN tx AS input_tx ON input_tx.hash_id = input.tx_hash_id
    JOIN block_tx AS input_block_tx ON input_block_tx.tx_hash_id = input_tx.hash_id
    JOIN block AS input_block ON input_block.hash_id = input_block_tx.block_hash_id
  ON output.tx_hash_id = input.output_tx_hash_id AND input_block.extinct = false
  WHERE
    output_block.extinct = false
  GROUP BY
    output.address;

CREATE OR REPLACE VIEW address_balance_at_height AS
  SELECT address, block.height, SUM(
    CASE WHEN output_block.height <= block.height AND input.output_tx_hash_id IS NULL THEN output.value ELSE 0 END
  ) AS value
  FROM block
  JOIN output ON true
  JOIN tx AS output_tx ON output_tx.hash_id = output.tx_hash_id
  JOIN block_tx AS output_block_tx ON output_block_tx.tx_hash_id = output_tx.hash_id
  JOIN block AS output_block ON output_block.hash_id = output_block_tx.block_hash_id
  LEFT JOIN input
    JOIN tx AS input_tx ON input_tx.hash_id = input.tx_hash_id
    JOIN block_tx AS input_block_tx ON input_block_tx.tx_hash_id = input_tx.hash_id
    JOIN block AS input_block ON input_block.hash_id = input_block_tx.block_hash_id
  ON output.tx_hash_id = input.output_tx_hash_id AND
    input_block.extinct = false AND
    input_block.height <= block.height
  WHERE
    block.extinct = false AND
    output_block.extinct = false
  GROUP BY
    block.height,
    output.address
  ORDER BY output.address;

--
-- Performance tuning
--

ANALYZE block;
ANALYZE block_tx;
ANALYZE tx;
ANALYZE output;
ANALYZE input;

-- enableautovacum: it might be useful anyway
ALTER TABLE event SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

ALTER TABLE block SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

ALTER TABLE block_tx SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

ALTER TABLE tx SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

ALTER TABLE output SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

ALTER TABLE input SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

ALTER TABLE input SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

