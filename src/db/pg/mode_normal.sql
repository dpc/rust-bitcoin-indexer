-- normal mode schema: after reaching chainhead/first reorg
-- we build all indices, etc. to enable all the queries etc.
CREATE INDEX IF NOT EXISTS block_extinct ON block (extinct);

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

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'mempool' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE mempool ADD PRIMARY KEY (tx_hash_id);
  END IF;
END $$;
CREATE INDEX IF NOT EXISTS mempool_ts ON mempool (ts);

ANALYZE block;
ANALYZE tx;
ANALYZE block_tx;
ANALYZE output;
ANALYZE input;

-- enableautovacum: it might be useful anyway
ALTER TABLE event SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

ALTER TABLE block SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

ALTER TABLE tx SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

ALTER TABLE block_tx SET (
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


-- tx joined all the way to the block
-- NOTE: there might be from 0 (NULL data),
-- to many blocks which happaned to include the tx (extinct blocks)
CREATE OR REPLACE VIEW tx_maybe_with_block AS
  SELECT tx.*,
  block.hash_id AS block_hash_id,
  block.hash_rest AS block_hash_rest,
  block.height AS block_height,
  block.prev_hash_id AS block_prev_hash_id,
  block.merkle_root AS block_merkle_root,
  block.time AS block_time,
  block.extinct AS block_extinct
  FROM tx
  LEFT JOIN block_tx
    JOIN block ON block.hash_id = block_tx.block_hash_id
  ON block_tx.tx_hash_id = tx.hash_id;

CREATE OR REPLACE VIEW tx_with_block AS
  SELECT * FROM tx_maybe_with_block WHERE block_hash_id IS NOT NULL;

-- txes in the mempool
-- it does ignore extinct blocks
CREATE OR REPLACE VIEW tx_in_mempool AS
  select
    tx.*,
    mempool.ts
  FROM tx
  JOIN mempool
    ON tx.hash_id = mempool.tx_hash_id
  left join tx_maybe_with_block
    on mempool.tx_hash_id = tx_maybe_with_block.hash_id
  where tx_maybe_with_block.block_hash_id IS NULL OR tx_maybe_with_block.block_extinct = true;

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
